//! macOS: IOKit's system power notices for sleep and wake, the
//! `com.apple.screenIsLocked` distributed notification, and NSWorkspace's
//! power-off notice (003 research.md §14).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
use objc2::{AnyThread, DefinedClass, class, define_class, ffi, msg_send, sel};
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
            // Before the event is handled on the system-events thread: the
            // quit that follows may already be asking `applicationShouldTerminate:`
            // (#107).
            POWER_OFF_NOTICED.store(true, Ordering::SeqCst);
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

/// Who asked the application to terminate (#107).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitSource {
    /// The user: ⌘Q, the application menu's or the Dock's Quit, a script's
    /// quit. It is a request, so the user is asked about unsaved changes.
    User,
    /// The OS is ending the session (log out, restart, shut down): 003 FR-039.
    System,
}

/// What the application delegate's `applicationShouldTerminate:` answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// The quit was a request that has been passed on; the app stays open.
    Cancel,
    /// Terminate at once.
    Now,
    /// Not yet: the hook has promised to call [`reply_to_terminate`] once it
    /// is done (NSTerminateLater). A log out waits for the reply, so the
    /// close happens before the process goes, and the log out isn't
    /// cancelled.
    Later,
}

/// Answers the application delegate's `applicationShouldTerminate:` from who
/// asked.
type TerminateDecision = Arc<dyn Fn(QuitSource) -> Termination + Send + Sync>;

static ON_TERMINATE: Mutex<Option<TerminateDecision>> = Mutex::new(None);

/// A `Termination::Later` answer that has not been replied to yet.
static LATER_PENDING: AtomicBool = AtomicBool::new(false);

/// NSWorkspace's power-off notice has arrived (a log out, restart or shut
/// down was confirmed), so any quit that follows is the OS's.
static POWER_OFF_NOTICED: AtomicBool = AtomicBool::new(false);

// NSApplicationTerminateReply
const TERMINATE_CANCEL: usize = 0;
const TERMINATE_NOW: usize = 1;
const TERMINATE_LATER: usize = 2;

/// `kAEQuitReason` (AERegistry.h): the parameter of the quit Apple Event
/// that says why it was sent. The Dock's Quit, a script and `terminate:`
/// from a key press or menu carry none.
const QUIT_REASON_KEYWORD: u32 = u32::from_be_bytes(*b"why?");

/// The values of `kAEQuitReason` that mean the OS is ending the session:
/// `kAELogOut`, `kAEReallyLogOut`, `kAEShowRestartDialog`, `kAERestart`,
/// `kAEShowShutdownDialog`, `kAEShutDown` and `kAEQuitAll` (what loginwindow
/// sends to every application as a session ends). `kAESleep` isn't a quit.
const SYSTEM_QUIT_REASONS: [[u8; 4]; 7] =
    [*b"logo", *b"rlgo", *b"rrst", *b"rest", *b"rsdn", *b"shut", *b"quia"];

/// Who is behind a quit, from its `kAEQuitReason` (none for a key press or
/// an event without one). The race this settles: the power-off notice only
/// queues an event for the system-events thread, which sets `OS_ENDING` later,
/// so a `terminate:` can arrive first, and cancelling it would cancel the
/// user's log out ("HoploDex interrupted log out") and skip 003 FR-039.
pub fn quit_source(reason: Option<u32>, power_off_noticed: bool) -> QuitSource {
    let by_reason = reason
        .is_some_and(|code| SYSTEM_QUIT_REASONS.iter().any(|r| u32::from_be_bytes(*r) == code));
    if by_reason || power_off_noticed { QuitSource::System } else { QuitSource::User }
}

/// The `kAEQuitReason` of a quit Apple Event, if it has one.
fn quit_reason_of(event: &AnyObject) -> Option<u32> {
    // SAFETY: `event` is an NSAppleEventDescriptor; both selectors return
    // an object or nil, and an OSType.
    unsafe {
        let parameter: *mut AnyObject =
            msg_send![event, paramDescriptorForKeyword: QUIT_REASON_KEYWORD];
        let parameter = parameter.as_ref()?;
        // loginwindow sends a type or an enumeration code, depending on the
        // reason; each reads as the other's accessor returns 0 for it.
        let code: u32 = msg_send![parameter, enumCodeValue];
        if code != 0 {
            return Some(code);
        }
        let code: u32 = msg_send![parameter, typeCodeValue];
        (code != 0).then_some(code)
    }
}

/// The reason of the Apple Event being handled, none when `terminate:` was
/// called directly (⌘Q, the menu) or the event carries none.
fn current_quit_reason() -> Option<u32> {
    // SAFETY: Foundation's shared manager; currentAppleEvent is nil outside
    // an Apple Event's handler.
    unsafe {
        let manager: *mut AnyObject =
            msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let event: *mut AnyObject = msg_send![manager, currentAppleEvent];
        quit_reason_of(event.as_ref()?)
    }
}

extern "C-unwind" fn application_should_terminate(
    _this: &AnyObject,
    _cmd: Sel,
    _sender: *mut AnyObject,
) -> usize {
    // A second `terminate:` while the first waits for its reply.
    if LATER_PENDING.load(Ordering::SeqCst) {
        return TERMINATE_LATER;
    }
    let source = quit_source(current_quit_reason(), POWER_OFF_NOTICED.load(Ordering::SeqCst));
    let decide = ON_TERMINATE.lock().ok().and_then(|hook| hook.clone());
    match decide.map_or(Termination::Now, |decide| decide(source)) {
        Termination::Cancel => TERMINATE_CANCEL,
        Termination::Now => TERMINATE_NOW,
        Termination::Later => {
            LATER_PENDING.store(true, Ordering::SeqCst);
            TERMINATE_LATER
        }
    }
}

unsafe extern "C" {
    // The main queue, which libdispatch runs in the run loop's common
    // modes, so also in the mode AppKit waits for a reply in.
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

extern "C" fn reply_on_main_thread(context: *mut c_void) {
    // SAFETY: `context` is the Box<bool> leaked by `reply_to_terminate`.
    let proceed = unsafe { *Box::from_raw(context as *mut bool) };
    // SAFETY: NSApplication's reply, on the main thread.
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, replyToApplicationShouldTerminate: Bool::new(proceed)];
    }
}

/// Completes a [`Termination::Later`]: terminate when `proceed`, else stay
/// open. Callable from any thread; it never blocks, and the reply runs on
/// the main thread when its run loop gets to it, so a caller that the main
/// thread is waiting for can't deadlock with it.
pub fn reply_to_terminate(proceed: bool) {
    LATER_PENDING.store(false, Ordering::SeqCst);
    let context = Box::into_raw(Box::new(proceed)) as *mut c_void;
    // SAFETY: the queue is libdispatch's main queue, and the function takes
    // the context it is given.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, context, reply_on_main_thread) };
}

/// Adds `applicationShouldTerminate:` to `class`, an application delegate's
/// class that doesn't define it, so that `terminate:` (⌘Q, the application
/// menu's and the Dock's Quit, an Apple Event quit) calls `decide` first and
/// goes on only when it returns true (#107). Returns false, adding nothing,
/// if the class already answers the selector. Call it before the run loop
/// starts.
fn add_terminate_hook(
    class: &AnyClass,
    decide: impl Fn(QuitSource) -> Termination + Send + Sync + 'static,
) -> bool {
    let selector = sel!(applicationShouldTerminate:);
    if class.responds_to(selector) {
        return false;
    }
    *ON_TERMINATE.lock().expect("the terminate hook's lock") = Some(Arc::new(decide));
    // SAFETY: the function takes (self, _cmd, sender) and returns an
    // NSUInteger, which is what the type encoding "Q@:@" says.
    let imp: Imp = unsafe {
        std::mem::transmute::<extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> usize, Imp>(
            application_should_terminate,
        )
    };
    // SAFETY: `class` is a registered class, and class_addMethod only adds to
    // it (it does not replace a method it already defines).
    unsafe {
        ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            selector,
            imp,
            c"Q@:@".as_ptr(),
        )
        .as_bool()
    }
}

/// Makes every `terminate:` of the application ask `decide` first (#107).
/// tao ends the app from `applicationWillTerminate:` without ever raising
/// `RunEvent::ExitRequested`, so the unsaved-changes question and the
/// database's normal close were skipped; its delegate doesn't define
/// `applicationShouldTerminate:`, so this adds it to the delegate's class.
/// `decide` is told who asked: for the user it passes the request on and
/// returns `Cancel` (the quit then comes back through `quit_application`,
/// which exits with a code through `stop:`, not `terminate:`); for the OS
/// ending the session (003 FR-039) it returns `Later`, closes off the main thread
/// and calls [`reply_to_terminate`], never cancelling the log out.
///
/// Call it once the event loop has been created (it sets the delegate) and
/// before it runs. Returns false, with a warning logged, when there is no
/// delegate or it already answers `applicationShouldTerminate:`.
pub fn ask_before_terminate(
    decide: impl Fn(QuitSource) -> Termination + Send + Sync + 'static,
) -> bool {
    // SAFETY: NSApplication's sharedApplication and delegate, on the main
    // thread (where `main` runs this).
    let class = unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let delegate: *mut AnyObject = msg_send![app, delegate];
        delegate.as_ref().map(AnyObject::class)
    };
    let Some(class) = class else {
        log::warn!("no application delegate: ⌘Q will not ask about unsaved changes");
        return false;
    };
    let added = add_terminate_hook(class, decide);
    if !added {
        log::warn!(
            "{} already answers applicationShouldTerminate:: Quit will not ask about unsaved changes",
            class.name().to_string_lossy()
        );
    }
    added
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

    /// The tests that set or depend on `POWER_OFF_NOTICED`, one at a time.
    static NOTICE_FLAG: Mutex<()> = Mutex::new(());

    #[test]
    fn the_power_off_notice_is_will_shut_down() {
        let _flag = NOTICE_FLAG.lock().unwrap_or_else(|e| e.into_inner());
        POWER_OFF_NOTICED.store(false, Ordering::SeqCst);
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
        // Noted on the posting thread itself, not left to the thread that
        // handles the event: a `terminate:` may follow at once (#107).
        assert!(POWER_OFF_NOTICED.swap(false, Ordering::SeqCst));
    }

    /// A delegate class like tao's, which has no `applicationShouldTerminate:` of its own.
    fn delegate_class() -> &'static AnyClass {
        use objc2::ClassType;
        use objc2::runtime::ClassBuilder;
        ClassBuilder::new(c"HoploDexTestTerminateDelegate", NSObject::class())
            .expect("a new class name")
            .register()
    }

    #[test]
    fn the_quit_reason_decides_who_asked() {
        let code = |reason: &[u8; 4]| Some(u32::from_be_bytes(*reason));
        // A key press, the menu and the Dock's Quit carry no reason.
        assert_eq!(quit_source(None, false), QuitSource::User);
        // Every reason loginwindow gives for ending the session.
        for reason in [b"logo", b"rlgo", b"rrst", b"rest", b"rsdn", b"shut", b"quia"] {
            assert_eq!(quit_source(code(reason), false), QuitSource::System, "{reason:?}");
        }
        // Sleep isn't a quit, and an unknown value is a user's.
        assert_eq!(quit_source(code(b"slep"), false), QuitSource::User);
        assert_eq!(quit_source(code(b"zzzz"), false), QuitSource::User);
        // The power-off notice alone settles it, whatever the event says.
        assert_eq!(quit_source(None, true), QuitSource::System);
        assert_eq!(quit_source(code(b"slep"), true), QuitSource::System);
    }

    /// An NSAppleEventDescriptor for a quit event, with the reason parameter
    /// built `with` (none for no reason).
    fn quit_event(
        with: Option<&dyn Fn(u32) -> Retained<AnyObject>>,
        reason: u32,
    ) -> Retained<AnyObject> {
        let code = |text: &[u8; 4]| u32::from_be_bytes(*text);
        unsafe {
            let target: Retained<AnyObject> = msg_send![
                class!(NSAppleEventDescriptor),
                descriptorWithProcessIdentifier: std::process::id() as i32
            ];
            let event: Retained<AnyObject> = msg_send![
                class!(NSAppleEventDescriptor),
                appleEventWithEventClass: code(b"aevt"),
                eventID: code(b"quit"),
                targetDescriptor: &*target,
                returnID: -1i16,
                transactionID: 0i32
            ];
            if let Some(build) = with {
                let parameter = build(reason);
                let _: () = msg_send![&*event, setParamDescriptor: &*parameter, forKeyword: QUIT_REASON_KEYWORD];
            }
            event
        }
    }

    #[test]
    fn the_reason_is_read_from_the_apple_event_as_an_enum_or_a_type() {
        let code = |text: &[u8; 4]| u32::from_be_bytes(*text);
        let as_enum = |value: u32| -> Retained<AnyObject> {
            unsafe { msg_send![class!(NSAppleEventDescriptor), descriptorWithEnumCode: value] }
        };
        let as_type = |value: u32| -> Retained<AnyObject> {
            unsafe { msg_send![class!(NSAppleEventDescriptor), descriptorWithTypeCode: value] }
        };
        assert_eq!(quit_reason_of(&quit_event(None, 0)), None);
        for build in [&as_enum as &dyn Fn(u32) -> Retained<AnyObject>, &as_type] {
            let event = quit_event(Some(build), code(b"rlgo"));
            assert_eq!(quit_reason_of(&event), Some(code(b"rlgo")));
            assert_eq!(quit_source(quit_reason_of(&event), false), QuitSource::System);
        }
    }

    #[test]
    fn the_terminate_hook_cancels_a_users_quit_and_waits_for_the_os() {
        let _flag = NOTICE_FLAG.lock().unwrap_or_else(|e| e.into_inner());
        POWER_OFF_NOTICED.store(false, Ordering::SeqCst);
        let asked = Arc::new(AtomicU32::new(0));
        let class = delegate_class();
        let asked_seen = asked.clone();
        assert!(add_terminate_hook(class, move |source| {
            asked_seen.fetch_add(1, Ordering::SeqCst);
            match source {
                QuitSource::User => Termination::Cancel,
                QuitSource::System => Termination::Later,
            }
        }));
        // A second hook on a class that has one is refused.
        assert!(!add_terminate_hook(class, |_| Termination::Now));

        let delegate: Retained<AnyObject> = unsafe { msg_send![class, new] };
        let terminate = |delegate: &AnyObject| -> usize {
            let null: *mut AnyObject = std::ptr::null_mut();
            unsafe { msg_send![delegate, applicationShouldTerminate: null] }
        };
        // A user's quit (no Apple Event is being handled) is cancelled.
        assert_eq!(terminate(&delegate), TERMINATE_CANCEL);
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        // The power-off notice makes the next one the OS's: NSTerminateLater,
        // and a second `terminate:` while waiting doesn't ask again.
        POWER_OFF_NOTICED.store(true, Ordering::SeqCst);
        assert_eq!(terminate(&delegate), TERMINATE_LATER);
        assert_eq!(terminate(&delegate), TERMINATE_LATER);
        assert_eq!(asked.load(Ordering::SeqCst), 2);
        POWER_OFF_NOTICED.store(false, Ordering::SeqCst);
        LATER_PENDING.store(false, Ordering::SeqCst);
    }

    #[test]
    fn the_real_screen_lock_notices_are_observed() {
        assert_eq!(SCREEN_LOCKED, "com.apple.screenIsLocked");
        assert_eq!(SCREEN_UNLOCKED, "com.apple.screenIsUnlocked");
    }
}
