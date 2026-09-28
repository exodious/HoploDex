//! The time as the session sees it: now, with the computer's time zone,
//! which decides what "today" is for the once-a-day backup (research.md §7).
//! The tests put a clock of their own in its place.

use chrono::{DateTime, FixedOffset, Local};

pub trait Clock: Send + Sync {
    /// Now, in the computer's local time zone.
    fn now(&self) -> DateTime<FixedOffset>;
}

/// The computer's clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<FixedOffset> {
        Local::now().fixed_offset()
    }
}
