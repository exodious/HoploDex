//! The operating system's sleep, wake, screen-lock and shutdown notices, as
//! one stream (research.md §14). Each OS backend runs on its own thread and
//! sends [`SystemEvent`]s; `main.rs` hands each to `session::lifecycle`. A
//! wake watchdog also notices a sleep the OS gave no usable notice of.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Tells the OS the application is ready for what it announced, when
/// dropped: a sleep or shutdown waits for it, within the OS's limit.
pub struct Ack(Option<Box<dyn FnOnce() + Send>>);

impl Ack {
    pub fn new(ready: impl FnOnce() + Send + 'static) -> Self {
        Self(Some(Box::new(ready)))
    }

    /// Nothing to tell: the OS doesn't wait.
    pub fn none() -> Self {
        Self(None)
    }
}

impl Drop for Ack {
    fn drop(&mut self) {
        if let Some(ready) = self.0.take() {
            ready();
        }
    }
}

impl std::fmt::Debug for Ack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ack")
    }
}

#[derive(Debug)]
pub enum SystemEvent {
    /// The computer is about to sleep or hibernate.
    WillSleep {
        ack: Ack,
    },
    /// It woke, or the watchdog found it had slept.
    Woke,
    ScreenLocked,
    ScreenUnlocked,
    /// The OS is shutting down or logging out.
    WillShutDown {
        ack: Ack,
    },
}

static SCREEN_LOCK_SUPPORTED: AtomicBool = AtomicBool::new(false);

/// Whether this desktop tells applications that the screen locked (FR-038),
/// as found when the listener started. Always false before then, as in the
/// tests.
pub fn screen_lock_supported() -> bool {
    SCREEN_LOCK_SUPPORTED.load(Ordering::SeqCst)
}

/// Starts the OS backend and the wake watchdog, each on its own thread,
/// sending to `sender`.
pub fn spawn_listener(sender: Sender<SystemEvent>) {
    #[cfg(target_os = "linux")]
    let supported = linux::spawn(sender.clone());
    #[cfg(target_os = "macos")]
    let supported = macos::spawn(sender.clone());
    #[cfg(windows)]
    let supported = windows::spawn(sender.clone());
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let supported = false;
    SCREEN_LOCK_SUPPORTED.store(supported, Ordering::SeqCst);
    spawn_watchdog(sender);
}

/// How far wall time may run ahead of monotonic time before it counts as
/// a sleep (research.md §14).
const WAKE_JUMP: Duration = Duration::from_secs(5);

/// Notices a sleep from the clocks alone: the monotonic clock stops while
/// the computer sleeps, and wall time does not.
pub struct WakeWatchdog {
    wall: SystemTime,
    monotonic: Instant,
}

impl WakeWatchdog {
    pub fn new(wall: SystemTime, monotonic: Instant) -> Self {
        Self { wall, monotonic }
    }

    /// Takes the clocks' new readings. Returns whether wall time ran more
    /// than 5 s ahead of monotonic time since the last ones.
    pub fn observe(&mut self, wall: SystemTime, monotonic: Instant) -> bool {
        let wall_passed = wall.duration_since(self.wall).unwrap_or_default();
        let monotonic_passed = monotonic.saturating_duration_since(self.monotonic);
        self.wall = wall;
        self.monotonic = monotonic;
        wall_passed.saturating_sub(monotonic_passed) > WAKE_JUMP
    }
}

fn spawn_watchdog(sender: Sender<SystemEvent>) {
    let spawned = thread::Builder::new().name("wake-watchdog".into()).spawn(move || {
        let mut watchdog = WakeWatchdog::new(SystemTime::now(), Instant::now());
        loop {
            thread::sleep(Duration::from_secs(1));
            if watchdog.observe(SystemTime::now(), Instant::now())
                && sender.send(SystemEvent::Woke).is_err()
            {
                return;
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the wake watchdog: {err}");
    }
}
