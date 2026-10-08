//! macOS: the screen-lock notices reach the session as `ScreenLocked` and
//! `ScreenUnlocked` (FR-038, research.md §14, #28).
//!
//! It posts under notice names of its own. The real ones,
//! `com.apple.screenIsLocked` and `com.apple.screenIsUnlocked`, go to every
//! process on the computer, so they would lock every HoploDex running there
//! and write its pending changes to its database. The observer, the
//! distributed centre and the delivery are the app's own.
//!
//! Distributed notices are delivered on the main thread's run loop, and
//! libtest runs each test on a thread of its own, so this test program has
//! its own `main` (`harness = false` in Cargo.toml). It answers nextest's
//! `--list` itself, and runs its one test on the main thread.

const NAME: &str = "screen_lock_notices_arrive_as_locked_and_unlocked";

fn main() {
    let (mut listing, mut ignored, mut filters, mut skips) = (false, false, vec![], vec![]);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => listing = true,
            "--ignored" => ignored = true,
            "--skip" => skips.extend(args.next()),
            // libtest's options that take a value.
            "--format" | "--test-threads" | "--color" | "--logfile" | "-Z" => {
                args.next();
            }
            _ if arg.starts_with('-') => {}
            _ => filters.push(arg),
        }
    }
    let selected = cfg!(target_os = "macos")
        && filters.iter().all(|filter| NAME.contains(filter.as_str()))
        && !skips.iter().any(|skip| NAME.contains(skip.as_str()));
    if listing {
        if selected && !ignored {
            println!("{NAME}: test");
        }
        return;
    }
    if !selected || ignored {
        return;
    }
    #[cfg(target_os = "macos")]
    macos::screen_lock_notices_arrive_as_locked_and_unlocked();
    println!("test {NAME} ... ok");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, Instant};

    use hoplodex_lib::platform::{SystemEvent, observe_notifications};
    use objc2_foundation::{NSDistributedNotificationCenter, NSString};

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source: u8) -> i32;
    }

    /// Runs the main thread's run loop, which delivers the notices, until an
    /// event arrives or 5 s pass.
    fn next_event(events: &Receiver<SystemEvent>) -> Option<SystemEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(event) = events.try_recv() {
                return Some(event);
            }
            // SAFETY: runs this thread's run loop for a moment.
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, 1) };
        }
        None
    }

    pub fn screen_lock_notices_arrive_as_locked_and_unlocked() {
        let locked = format!("io.github.exodious.HoploDex.test.{}.locked", std::process::id());
        let unlocked = format!("io.github.exodious.HoploDex.test.{}.unlocked", std::process::id());
        let (sender, events) = mpsc::channel();
        observe_notifications(sender, &locked, &unlocked);
        let center = NSDistributedNotificationCenter::defaultCenter();

        // SAFETY: posts a notice with no object.
        unsafe { center.postNotificationName_object(&NSString::from_str(&locked), None) };
        let event = next_event(&events);
        assert!(matches!(event, Some(SystemEvent::ScreenLocked)), "got {event:?}");
        // SAFETY: as above.
        unsafe { center.postNotificationName_object(&NSString::from_str(&unlocked), None) };
        let event = next_event(&events);
        assert!(matches!(event, Some(SystemEvent::ScreenUnlocked)), "got {event:?}");
    }
}
