//! Windows: power, session and shutdown messages, received by one hidden,
//! never-shown top-level window with its own message loop on its own thread
//! (research.md §14). Broadcasts go only to top-level windows, so a
//! message-only (`HWND_MESSAGE`) window would miss them.

use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows_sys::Win32::System::Shutdown::{ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PBT_APMRESUMEAUTOMATIC,
    PBT_APMSUSPEND, RegisterClassW, TranslateMessage, WM_CLOSE, WM_ENDSESSION, WM_POWERBROADCAST,
    WM_QUERYENDSESSION, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
    WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use super::{Ack, SystemEvent};

/// Where the window procedure sends what it hears.
static SENDER: OnceLock<Mutex<Sender<SystemEvent>>> = OnceLock::new();

/// About as long as Windows lets an application hold up a sleep; a
/// shutdown allows a little longer.
const SLEEP_WAIT: Duration = Duration::from_secs(2);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

/// The hidden window's class, and its title.
const CLASS_NAME: &str = "HoploDexSystemMessages";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn send(event: SystemEvent) {
    if let Some(Ok(sender)) = SENDER.get().map(Mutex::lock) {
        let _ = sender.send(event);
    }
}

/// Sends an event that the OS waits for, and waits, up to `limit`, for its
/// ack: the window procedure must not return before the lock is done.
fn send_and_wait(make: impl FnOnce(Ack) -> SystemEvent, limit: Duration) {
    let (done, finished) = mpsc::channel::<()>();
    send(make(Ack::new(move || {
        let _ = done.send(());
    })));
    let _ = finished.recv_timeout(limit);
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_POWERBROADCAST => {
            match wparam as u32 {
                PBT_APMSUSPEND => send_and_wait(|ack| SystemEvent::WillSleep { ack }, SLEEP_WAIT),
                PBT_APMRESUMEAUTOMATIC => send(SystemEvent::Woke),
                _ => {}
            }
            1
        }
        WM_WTSSESSION_CHANGE => {
            match wparam as u32 {
                WTS_SESSION_LOCK => send(SystemEvent::ScreenLocked),
                WTS_SESSION_UNLOCK => send(SystemEvent::ScreenUnlocked),
                _ => {}
            }
            0
        }
        // The session may end: let it.
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if wparam != 0 {
                // Shown by Windows while pending changes are saved.
                let reason = wide("Saving unsaved changes and locking the database");
                // SAFETY: `window` is this thread's own window, and `reason`
                // is a NUL-terminated string that outlives the call.
                unsafe { ShutdownBlockReasonCreate(window, reason.as_ptr()) };
                send_and_wait(|ack| SystemEvent::WillShutDown { ack }, SHUTDOWN_WAIT);
                // SAFETY: `window` is this thread's own window.
                unsafe { ShutdownBlockReasonDestroy(window) };
            }
            0
        }
        // `taskkill` without /F, and the Restart Manager, ask every top-level
        // window of the process to close. The app's own window answers for
        // the app (and may ask about unsaved changes); this one stays, or the
        // notices would stop for the rest of the run.
        WM_CLOSE => 0,
        // SAFETY: the arguments are the ones Windows gave this procedure.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Starts the window's thread. The screen lock is always reported on
/// Windows.
pub fn spawn(sender: Sender<SystemEvent>) -> bool {
    if SENDER.set(Mutex::new(sender)).is_err() {
        return true;
    }
    let spawned = thread::Builder::new().name("system-messages".into()).spawn(|| {
        let class_name = wide(CLASS_NAME);
        // SAFETY: the class and window names outlive the calls, and the
        // window is used only on this thread.
        unsafe {
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_procedure),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: std::ptr::null_mut(),
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            if RegisterClassW(&class) == 0 {
                return log::warn!("could not register the system-messages window class");
            }
            // Top-level, never shown, and kept off the taskbar.
            let window = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                class_name.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            if window.is_null() {
                return log::warn!("could not create the system-messages window");
            }
            if WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) == 0 {
                log::warn!("could not register for screen-lock notices");
            }
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the system-messages window: {err}");
    }
    true
}

#[cfg(test)]
mod tests {
    //! The window procedure, driven through the real hidden window with
    //! synthetic messages, as Windows sends them (#27). That Windows
    //! delivers them at a real sleep, lock or log-off is checked by hand.

    use std::sync::mpsc::Receiver;
    use std::sync::{MutexGuard, PoisonError};
    use std::time::Instant;

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        ENDSESSION_LOGOFF, FindWindowExW, GetWindowThreadProcessId, IsWindow,
        PBT_APMPOWERSTATUSCHANGE, SendMessageW, WTS_CONSOLE_CONNECT,
    };

    use super::*;

    /// How long a test's lock holds up a sleep or log-off.
    const HELD: Duration = Duration::from_millis(300);

    struct Listener {
        /// The hidden window, as a number so it can cross threads.
        window: usize,
        events: Mutex<Receiver<SystemEvent>>,
    }

    /// What the process's one hidden window sends. Each test holds this
    /// throughout, so tests that share a process take turns.
    fn events() -> MutexGuard<'static, Receiver<SystemEvent>> {
        listener().events.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The hidden window, started on first use.
    fn listener() -> &'static Listener {
        static LISTENER: OnceLock<Listener> = OnceLock::new();
        LISTENER.get_or_init(|| {
            let (sender, events) = mpsc::channel();
            assert!(spawn(sender));
            Listener { window: find_window(), events: Mutex::new(events) }
        })
    }

    /// This process's window of [`CLASS_NAME`], once its thread has made
    /// it. Other processes (tests run side by side) have windows of the
    /// same class.
    fn find_window() -> usize {
        let class = wide(CLASS_NAME);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let mut window: HWND = std::ptr::null_mut();
            loop {
                // SAFETY: `class` is NUL-terminated and outlives the call;
                // `window` is null or a window the last call returned.
                window = unsafe {
                    FindWindowExW(std::ptr::null_mut(), window, class.as_ptr(), std::ptr::null())
                };
                if window.is_null() {
                    break;
                }
                let mut process = 0;
                // SAFETY: `process` outlives the call.
                unsafe { GetWindowThreadProcessId(window, &mut process) };
                if process == std::process::id() {
                    return window as usize;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the system-messages window never appeared");
    }

    /// Sends a message the way Windows does, from another thread, waiting
    /// for the window procedure. Joined, it gives what the procedure
    /// returned and how long that took.
    fn send(message: u32, wparam: u32, lparam: u32) -> thread::JoinHandle<(LRESULT, Duration)> {
        let window = listener().window;
        thread::spawn(move || {
            let started = Instant::now();
            // SAFETY: the window lasts as long as the process, and the
            // arguments are plain numbers.
            let returned = unsafe {
                SendMessageW(window as HWND, message, wparam as WPARAM, lparam as LPARAM)
            };
            (returned, started.elapsed())
        })
    }

    /// [`send`], joined, for a message nothing waits on.
    fn returned(message: u32, wparam: u32, lparam: u32) -> LRESULT {
        send(message, wparam, lparam).join().expect("the sending thread panicked").0
    }

    fn next(events: &Receiver<SystemEvent>) -> SystemEvent {
        events.recv_timeout(Duration::from_secs(5)).expect("the window sent nothing")
    }

    /// Checks that the messages before sent nothing: a resume's wake comes
    /// next.
    fn assert_nothing_sent(events: &Receiver<SystemEvent>) {
        assert_eq!(returned(WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC, 0), 1);
        let event = next(events);
        assert!(matches!(event, SystemEvent::Woke), "expected only a wake, got {event:?}");
    }

    #[test]
    fn a_suspend_is_a_sleep_that_waits_for_the_lock() {
        let events = events();
        let sent = send(WM_POWERBROADCAST, PBT_APMSUSPEND, 0);
        let ack = match next(&events) {
            SystemEvent::WillSleep { ack } => ack,
            other => panic!("expected WillSleep, got {other:?}"),
        };
        thread::sleep(HELD);
        drop(ack);
        let (returned, took) = sent.join().unwrap();
        assert_eq!(returned, 1);
        assert!(took >= HELD, "the suspend returned after {took:?}, before the lock was done");
        assert!(took < SLEEP_WAIT, "the suspend waited {took:?}, past the lock");
    }

    #[test]
    fn a_sleep_waits_no_longer_than_windows_allows() {
        let events = events();
        let sent = send(WM_POWERBROADCAST, PBT_APMSUSPEND, 0);
        let ack = match next(&events) {
            SystemEvent::WillSleep { ack } => ack,
            other => panic!("expected WillSleep, got {other:?}"),
        };
        let (returned, took) = sent.join().unwrap();
        drop(ack);
        assert_eq!(returned, 1);
        assert!(
            (SLEEP_WAIT..SLEEP_WAIT + Duration::from_secs(1)).contains(&took),
            "a lock that never finished held up the suspend for {took:?}"
        );
    }

    #[test]
    fn a_resume_is_a_wake() {
        let events = events();
        assert_nothing_sent(&events);
    }

    #[test]
    fn other_power_notices_send_nothing() {
        let events = events();
        assert_eq!(returned(WM_POWERBROADCAST, PBT_APMPOWERSTATUSCHANGE, 0), 1);
        assert_nothing_sent(&events);
    }

    #[test]
    fn a_session_lock_and_unlock_are_the_screen_lock() {
        let events = events();
        assert_eq!(returned(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, 1), 0);
        let event = next(&events);
        assert!(matches!(event, SystemEvent::ScreenLocked), "got {event:?}");
        assert_eq!(returned(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK, 1), 0);
        let event = next(&events);
        assert!(matches!(event, SystemEvent::ScreenUnlocked), "got {event:?}");
    }

    #[test]
    fn other_session_changes_send_nothing() {
        let events = events();
        assert_eq!(returned(WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT, 1), 0);
        assert_nothing_sent(&events);
    }

    #[test]
    fn a_log_off_is_let_through_and_waits_for_the_close() {
        let events = events();
        // Asked first: the app never holds it up.
        assert_eq!(returned(WM_QUERYENDSESSION, 0, ENDSESSION_LOGOFF), 1);
        let sent = send(WM_ENDSESSION, 1, ENDSESSION_LOGOFF);
        let ack = match next(&events) {
            SystemEvent::WillShutDown { ack } => ack,
            other => panic!("expected WillShutDown, got {other:?}"),
        };
        thread::sleep(HELD);
        drop(ack);
        let (returned, took) = sent.join().unwrap();
        assert_eq!(returned, 0);
        assert!(took >= HELD, "the session ended after {took:?}, before the close was done");
        assert!(took < SHUTDOWN_WAIT, "the session end waited {took:?}, past the close");
    }

    #[test]
    fn a_cancelled_log_off_sends_nothing() {
        let events = events();
        // Another application refused it, so the session goes on.
        assert_eq!(returned(WM_ENDSESSION, 0, ENDSESSION_LOGOFF), 0);
        assert_nothing_sent(&events);
    }

    #[test]
    fn a_request_to_close_leaves_the_window_listening() {
        let events = events();
        // What `taskkill` without /F sends to every top-level window.
        assert_eq!(returned(WM_CLOSE, 0, 0), 0);
        // SAFETY: a plain check of a window handle.
        assert_ne!(unsafe { IsWindow(listener().window as HWND) }, 0);
        assert_nothing_sent(&events);
    }
}
