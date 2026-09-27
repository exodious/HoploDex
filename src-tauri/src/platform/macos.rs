//! macOS: IOKit's system power notices for sleep and wake, the
//! `com.apple.screenIsLocked` distributed notification, and NSWorkspace's
//! power-off notice (research.md §14).

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::thread;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::{NSWorkspace, NSWorkspaceWillPowerOffNotification};
use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSString};

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
    sender: Mutex<Sender<SystemEvent>>,
    root_port: Mutex<IoConnect>,
}

impl Power {
    fn send(&self, event: SystemEvent) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(event);
        }
    }

    fn root_port(&self) -> IoConnect {
        self.root_port.lock().map(|port| *port).unwrap_or(0)
    }
}

extern "C" fn power_changed(
    refcon: *mut c_void,
    _service: IoObject,
    message: u32,
    argument: *mut c_void,
) {
    // SAFETY: `refcon` is the leaked `Power` given at registration.
    let power = unsafe { &*(refcon as *const Power) };
    let root_port = power.root_port();
    let notification_id = argument as isize;
    match message {
        // Never veto an idle sleep.
        CAN_SYSTEM_SLEEP => unsafe {
            IOAllowPowerChange(root_port, notification_id);
        },
        // The system waits (up to 30 s) until the lock lets it go on.
        SYSTEM_WILL_SLEEP => power.send(SystemEvent::WillSleep {
            ack: Ack::new(move || unsafe {
                IOAllowPowerChange(root_port, notification_id);
            }),
        }),
        SYSTEM_HAS_POWERED_ON => power.send(SystemEvent::Woke),
        _ => {}
    }
}

/// Registers for system power notices and runs their run loop, on its own
/// thread.
fn listen_for_power(sender: Sender<SystemEvent>) {
    let spawned = thread::Builder::new().name("iokit-power".into()).spawn(move || {
        let power: &'static Power =
            Box::leak(Box::new(Power { sender: Mutex::new(sender), root_port: Mutex::new(0) }));
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
            if let Ok(mut held) = power.root_port.lock() {
                *held = root_port;
            }
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
    #[ivars = Mutex<Sender<SystemEvent>>]
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
        let this = Self::alloc().set_ivars(Mutex::new(sender));
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }

    fn send(&self, event: SystemEvent) {
        if let Ok(sender) = self.ivars().lock() {
            let _ = sender.send(event);
        }
    }
}

/// Starts the listeners. The screen lock is always reported on macOS.
pub fn spawn(sender: Sender<SystemEvent>) -> bool {
    listen_for_power(sender.clone());
    // Kept for the life of the process: the centres don't retain observers.
    // SAFETY: leaked, so valid for the life of the process.
    let observer: &'static Observer = unsafe { &*Retained::into_raw(Observer::new(sender)) };
    let observer: &AnyObject = observer;
    let distributed = NSDistributedNotificationCenter::defaultCenter();
    // SAFETY: the observer implements these selectors, taking the
    // notification, and outlives the registrations.
    unsafe {
        distributed.addObserver_selector_name_object(
            observer,
            sel!(screenLocked:),
            Some(&NSString::from_str("com.apple.screenIsLocked")),
            None,
        );
        distributed.addObserver_selector_name_object(
            observer,
            sel!(screenUnlocked:),
            Some(&NSString::from_str("com.apple.screenIsUnlocked")),
            None,
        );
        NSWorkspace::sharedWorkspace().notificationCenter().addObserver_selector_name_object(
            observer,
            sel!(willPowerOff:),
            Some(NSWorkspaceWillPowerOffNotification),
            None,
        );
    }
    true
}
