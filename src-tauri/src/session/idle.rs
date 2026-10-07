//! The idle lock's clock (FR-034, FR-035, research.md §15). It lives in the
//! backend, since a hidden or minimized webview's timers can be throttled or
//! frozen, and runs on wall time, so time asleep counts as idle. The
//! frontend reports input with `note_activity` at most once a second; a
//! 1 s tick locks once the open database's idle duration has passed since
//! the last input. A running operation, or a native file or folder dialog,
//! pauses it, and the idle time starts again from zero once it is over.

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, FixedOffset};

use crate::models::database::{IdlePauseReason, LockSettings};

#[derive(Default)]
struct State {
    /// The open database's lock settings; `None` while nothing is open,
    /// when there is nothing to lock.
    settings: Option<LockSettings>,
    /// The last input, or the moment the idle time last started again.
    last_input: Option<DateTime<FixedOffset>>,
    /// What the frontend has paused it for.
    paused_by: BTreeSet<&'static str>,
    /// An operation was running at the last tick.
    operation_was_running: bool,
}

/// Seconds in the idle lock's minute.
const SECONDS_PER_MINUTE: i64 = 60;

/// How long the idle lock counts as one minute: always 60 s, except that an
/// E2E build (`e2e` feature) reads `HOPLODEX_E2E_IDLE_MINUTE_SECONDS`, so a
/// spec can see the lock in seconds. A missing, unparsable or non-positive
/// value leaves it at 60 s.
#[cfg(feature = "e2e")]
fn configured_minute_seconds() -> i64 {
    minute_seconds_from(std::env::var("HOPLODEX_E2E_IDLE_MINUTE_SECONDS").ok().as_deref())
}

#[cfg(not(feature = "e2e"))]
fn configured_minute_seconds() -> i64 {
    SECONDS_PER_MINUTE
}

/// [`configured_minute_seconds`] with the setting given, so tests don't
/// touch the process environment.
#[cfg(any(test, feature = "e2e"))]
fn minute_seconds_from(setting: Option<&str>) -> i64 {
    setting
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(SECONDS_PER_MINUTE)
}

/// Input Rust itself sees in the application's windows, as the PDF surface's
/// web view does (research.md §10): counts as activity, like the main web
/// view's `note_activity`. The caller keeps it to once a second.
pub fn note_activity(session: &crate::session::Session) {
    session.idle().note_activity(session.clock().now());
}

/// Kept in the session.
pub struct IdleClock {
    state: Mutex<State>,
    /// Seconds one idle minute lasts; see [`configured_minute_seconds`].
    minute_seconds: i64,
}

impl Default for IdleClock {
    fn default() -> Self {
        Self::with_minute_seconds(configured_minute_seconds())
    }
}

impl IdleClock {
    fn with_minute_seconds(minute_seconds: i64) -> Self {
        Self { state: Mutex::default(), minute_seconds }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("idle clock mutex poisoned")
    }

    /// A database opened with `settings`, or its settings changed: the idle
    /// time starts from `now`.
    pub fn start(&self, settings: LockSettings, now: DateTime<FixedOffset>) {
        let mut state = self.lock();
        state.settings = Some(settings);
        state.last_input = Some(now);
    }

    /// The database closed: nothing to lock until the next open.
    pub fn stop(&self) {
        let mut state = self.lock();
        state.settings = None;
        state.last_input = None;
        state.paused_by.clear();
    }

    /// Input to the application's windows.
    pub fn note_activity(&self, now: DateTime<FixedOffset>) {
        let mut state = self.lock();
        if state.settings.is_some() {
            state.last_input = Some(now);
        }
    }

    /// Pauses or resumes the clock for `reason`. Resuming starts the idle
    /// time again from `now` (spec edge case: "the idle time starts once it
    /// finishes").
    pub fn set_paused(&self, reason: IdlePauseReason, paused: bool, now: DateTime<FixedOffset>) {
        let mut state = self.lock();
        if paused {
            state.paused_by.insert(reason.as_str());
        } else if state.paused_by.remove(reason.as_str()) && state.settings.is_some() {
            state.last_input = Some(now);
        }
    }

    /// Whether the idle lock should lock now. `operation_running` pauses it
    /// like a native dialog does, from the operations registry.
    pub fn tick(&self, now: DateTime<FixedOffset>, operation_running: bool) -> bool {
        let mut state = self.lock();
        let was_running = std::mem::replace(&mut state.operation_was_running, operation_running);
        let Some(settings) = state.settings.clone() else { return false };
        if operation_running || !state.paused_by.is_empty() {
            return false;
        }
        if was_running {
            state.last_input = Some(now);
            return false;
        }
        if !settings.idle_enabled {
            return false;
        }
        let last_input = *state.last_input.get_or_insert(now);
        now - last_input >= chrono::Duration::seconds(settings.idle_minutes * self.minute_seconds)
    }

    /// The open database's idle duration, for the lock's notice.
    pub fn idle_minutes(&self) -> Option<i64> {
        self.lock().settings.as_ref().map(|settings| settings.idle_minutes)
    }

    /// The open database's lock settings, if one is open.
    pub fn settings(&self) -> Option<LockSettings> {
        self.lock().settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-01T09:00:00+00:00").unwrap()
            + chrono::Duration::seconds(seconds)
    }

    fn settings(idle_minutes: i64) -> LockSettings {
        LockSettings { idle_enabled: true, idle_minutes, on_screen_lock: false }
    }

    #[test]
    fn the_setting_scales_the_minute_and_anything_unusable_leaves_it_at_sixty_seconds() {
        assert_eq!(minute_seconds_from(Some("3")), 3);
        assert_eq!(minute_seconds_from(Some(" 5 ")), 5);
        for unusable in [None, Some(""), Some("soon"), Some("0"), Some("-4"), Some("1.5")] {
            assert_eq!(minute_seconds_from(unusable), 60, "{unusable:?}");
        }
    }

    #[test]
    fn a_scaled_minute_locks_after_that_many_seconds_per_minute() {
        let clock = IdleClock::with_minute_seconds(3);
        clock.start(settings(2), at(0));
        assert!(!clock.tick(at(5), false), "two scaled minutes are six seconds");
        assert!(clock.tick(at(6), false));
        assert_eq!(clock.idle_minutes(), Some(2), "the notice still counts real minutes");
    }

    #[test]
    fn an_unscaled_clock_still_waits_whole_minutes() {
        let clock = IdleClock::with_minute_seconds(60);
        clock.start(settings(1), at(0));
        assert!(!clock.tick(at(59), false));
        assert!(clock.tick(at(60), false));
    }
}
