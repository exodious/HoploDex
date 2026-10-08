//! macOS: IOKit's system power notices for sleep and wake, the
//! `com.apple.screenIsLocked` distributed notification, and NSWorkspace's
//! power-off notice (research.md §14).

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::{NSWorkspace, NSWorkspaceWillPowerOffNotification};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationSuspensionBehavior, NSString,
};

use super::{Ack, SystemEvent};

type IoObject = u32;
type IoConnect = u32;
type IoReturn = i32;
type NotificationPortRef = *mut c_void;
type CfRunLoopSourceRef = *const c_void;
type CfRunLoopRef = *const c_void;
type CfStringRef = *const c_void;
type PowerCallback = extern "C" fn(*mut c_void, IoObject, u32, *mut c_void);

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IORegisterForSystemPower(
        refcon: *mut c_void,
        port: *mut NotificationPortRef,
        callback: PowerCallback,
        notifier: *mut IoObject,
    ) -> IoConnect;
    fn IOAllowPowerChange(kernel_port: IoConnect, notification_id: isize) -> IoReturn;
    fn IODeregisterForSystemPower(notifier: *mut IoObject) -> IoReturn;
    fn IONotificationPortGetRunLoopSource(port: NotificationPortRef) -> CfRunLoopSourceRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: CfStringRef;
    fn CFRunLoopGetCurrent() -> CfRunLoopRef;
    fn CFRunLoopAddSource(run_loop: CfRunLoopRef, source: CfRunLoopSourceRef, mode: CfStringRef);
    fn CFRunLoopRun();
}

// IOMessage.h
const CAN_SYSTEM_SLEEP: u32 = 0xE000_0270;
const SYSTEM_WILL_SLEEP: u32 = 0xE000_0280;
const SYSTEM_HAS_POWERED_ON: u32 = 0xE000_0300;

/// What the power callback needs, kept for the life of the process.
struct Power {
    sender: Sender<SystemEvent>,
    root_port: AtomicU32,
}

extern "C" fn power_changed(
    refcon: *mut c_void,
    _service: IoObject,
    message: u32,
    argument: *mut c_void,
) {
    // SAFETY: `refcon` is the leaked `Power` given at registration.
    let power = unsafe { &*(refcon as *const Power) };
    let root_port = power.root_port.load(Ordering::SeqCst);
    let notification_id = argument as isize;
    match message {
        // Never veto an idle sleep.
        CAN_SYSTEM_SLEEP => unsafe {
            IOAllowPowerChange(root_port, notification_id);
        },
        // The system waits (up to 30 s) until the lock lets it go on.
        SYSTEM_WILL_SLEEP => {
            let _ = power.sender.send(SystemEvent::WillSleep {
                ack: Ack::new(move || unsafe {
                    IOAllowPowerChange(root_port, notification_id);
                }),
            });
        }
        SYSTEM_HAS_POWERED_ON => {
            let _ = power.sender.send(SystemEvent::Woke);
        }
        _ => {}
    }
}

/// Registers for system power notices and runs their run loop, on its own
/// thread.
fn listen_for_power(sender: Sender<SystemEvent>) {
    let spawned = thread::Builder::new().name("iokit-power".into()).spawn(move || {
        let power: &'static Power =
            Box::leak(Box::new(Power { sender, root_port: AtomicU32::new(0) }));
        let mut port: NotificationPortRef = std::ptr::null_mut();
        let mut notifier: IoObject = 0;
        // SAFETY: the out-pointers are valid, and `power` lives for the
        // life of the process, as the registration does.
        unsafe {
            let root_port = IORegisterForSystemPower(
                power as *const Power as *mut c_void,
                &mut port,
                power_changed,
                &mut notifier,
            );
            if root_port == 0 {
                return log::warn!("could not register for sleep notices");
            }
            // Notices arrive only through the run loop, so after this.
            power.root_port.store(root_port, Ordering::SeqCst);
            CFRunLoopAddSource(
                CFRunLoopGetCurrent(),
                IONotificationPortGetRunLoopSource(port),
                kCFRunLoopDefaultMode,
            );
            CFRunLoopRun();
            // The run loop only returns if it has no sources left.
            IODeregisterForSystemPower(&mut notifier);
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the sleep listener: {err}");
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class
    // doesn't implement `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = Sender<SystemEvent>]
    struct Observer;

    impl Observer {
        #[unsafe(method(screenLocked:))]
        fn screen_locked(&self, _notification: &NSNotification) {
            self.send(SystemEvent::ScreenLocked);
        }

        #[unsafe(method(screenUnlocked:))]
        fn screen_unlocked(&self, _notification: &NSNotification) {
            self.send(SystemEvent::ScreenUnlocked);
        }

        #[unsafe(method(willPowerOff:))]
        fn will_power_off(&self, _notification: &NSNotification) {
            self.send(SystemEvent::WillShutDown { ack: Ack::none() });
        }
    }
);

impl Observer {
    fn new(sender: Sender<SystemEvent>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(sender);
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }

    fn send(&self, event: SystemEvent) {
        let _ = self.ivars().send(event);
    }
}

/// The distributed notices loginwindow posts when the screen locks and
/// unlocks.
const SCREEN_LOCKED: &str = "com.apple.screenIsLocked";
const SCREEN_UNLOCKED: &str = "com.apple.screenIsUnlocked";

/// Starts the listeners. The screen lock is always reported on macOS.
pub fn spawn(sender: Sender<SystemEvent>) -> bool {
    listen_for_power(sender.clone());
    observe_notifications(sender, SCREEN_LOCKED, SCREEN_UNLOCKED);
    true
}

/// Observes the screen-lock notices, under the names given, and NSWorkspace's
/// power-off notice. `tests/macos_screen_lock_test.rs` gives names of its
/// own: posting the real ones would lock every HoploDex running on the
/// computer, and write their pending changes to their databases.
pub fn observe_notifications(sender: Sender<SystemEvent>, locked: &str, unlocked: &str) {
    // Kept for the life of the process: the centres don't retain observers.
    // SAFETY: leaked, so valid for the life of the process.
    let observer: &'static Observer = unsafe { &*Retained::into_raw(Observer::new(sender)) };
    let observer: &AnyObject = observer;
    let distributed = NSDistributedNotificationCenter::defaultCenter();
    // AppKit suspends distributed notices while the application isn't
    // active, and by default holds them until it is again: a lock while
    // HoploDex is in the background must arrive when it happens.
    let immediately = NSNotificationSuspensionBehavior::DeliverImmediately;
    // SAFETY: the observer implements these selectors, taking the
    // notification, and outlives the registrations.
    unsafe {
        distributed.addObserver_selector_name_object_suspensionBehavior(
            observer,
            sel!(screenLocked:),
            Some(&NSString::from_str(locked)),
            None,
            immediately,
        );
        distributed.addObserver_selector_name_object_suspensionBehavior(
            observer,
            sel!(screenUnlocked:),
            Some(&NSString::from_str(unlocked)),
            None,
            immediately,
        );
        NSWorkspace::sharedWorkspace().notificationCenter().addObserver_selector_name_object(
            observer,
            sel!(willPowerOff:),
            Some(NSWorkspaceWillPowerOffNotification),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    /// The IOKit callback with a message, as the kernel would call it.
    fn power_message(message: u32) -> Option<SystemEvent> {
        let (sender, events) = mpsc::channel();
        let power = Power { sender, root_port: AtomicU32::new(0) };
        power_changed(&power as *const Power as *mut c_void, 0, message, std::ptr::null_mut());
        events.try_recv().ok()
    }

    #[test]
    fn a_sleep_notice_is_will_sleep_and_a_power_on_notice_is_woke() {
        assert!(matches!(power_message(SYSTEM_WILL_SLEEP), Some(SystemEvent::WillSleep { .. })));
        assert!(matches!(power_message(SYSTEM_HAS_POWERED_ON), Some(SystemEvent::Woke)));
    }

    #[test]
    fn the_question_whether_the_system_may_sleep_is_answered_without_an_event() {
        assert!(power_message(CAN_SYSTEM_SLEEP).is_none());
    }

    #[test]
    fn the_power_off_notice_is_will_shut_down() {
        let (sender, events) = mpsc::channel();
        observe_notifications(sender, "unused.locked", "unused.unlocked");

        // NSWorkspace's centre is the process's own, so posting to it reaches
        // only this test. It delivers at once, on this thread.
        // SAFETY: posts a notice with no object.
        unsafe {
            NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .postNotificationName_object(NSWorkspaceWillPowerOffNotification, None)
        };
        assert!(matches!(events.try_recv(), Ok(SystemEvent::WillShutDown { .. })));
    }

    #[test]
    fn the_real_screen_lock_notices_are_observed() {
        assert_eq!(SCREEN_LOCKED, "com.apple.screenIsLocked");
        assert_eq!(SCREEN_UNLOCKED, "com.apple.screenIsUnlocked");
    }
}
