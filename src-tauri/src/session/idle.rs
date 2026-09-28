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

/// Kept in the session.
#[derive(Default)]
pub struct IdleClock {
    state: Mutex<State>,
}

impl IdleClock {
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
        now - last_input >= chrono::Duration::minutes(settings.idle_minutes)
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
