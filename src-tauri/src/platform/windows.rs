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
    PBT_APMSUSPEND, RegisterClassW, TranslateMessage, WM_ENDSESSION, WM_POWERBROADCAST,
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
        let class_name = wide("HoploDexSystemMessages");
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
