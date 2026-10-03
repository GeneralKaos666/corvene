//! Port of GHD `lib/offset-from.ts`: a time moved by a number of seconds,
//! minutes, hours, days or (365-day) years. GHD's overloads return the kind
//! of time they are given (milliseconds since the epoch for a `number`, a
//! `Date` for a `Date`); here that is [`Dateish`], implemented for `i64`
//! milliseconds and [`SystemTime`].

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// GHD `Unit` (`keyof typeof units`), singular and plural alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Year,
    Years,
    Day,
    Days,
    Hour,
    Hours,
    Minute,
    Minutes,
    Second,
    Seconds,
}

impl Unit {
    /// The unit in milliseconds (GHD `units`; a year is 365 days).
    pub fn millis(self) -> i64 {
        match self {
            Unit::Year | Unit::Years => 31_536_000_000,
            Unit::Day | Unit::Days => 86_400_000,
            Unit::Hour | Unit::Hours => 3_600_000,
            Unit::Minute | Unit::Minutes => 60_000,
            Unit::Second | Unit::Seconds => 1_000,
        }
    }
}

/// GHD `Dateish`: a time that can be moved by milliseconds and keeps its
/// kind.
pub trait Dateish: Sized {
    /// The time `millis` milliseconds later (earlier when negative).
    fn add_millis(self, millis: i64) -> Self;
}

/// Milliseconds since the epoch (`Date.now()`, GHD's `number`).
impl Dateish for i64 {
    fn add_millis(self, millis: i64) -> Self {
        self.saturating_add(millis)
    }
}

/// GHD's `Date`.
impl Dateish for SystemTime {
    fn add_millis(self, millis: i64) -> Self {
        let delta = Duration::from_millis(millis.unsigned_abs());
        let moved = if millis < 0 {
            self.checked_sub(delta)
        } else {
            self.checked_add(delta)
        };
        moved.unwrap_or(self)
    }
}

/// GHD `offsetFrom(date, value, unit)`: `date` moved by `value` `unit`s,
/// of the same kind as `date`.
pub fn offset_from<D: Dateish>(date: D, value: i64, unit: Unit) -> D {
    date.add_millis(value.saturating_mul(unit.millis()))
}

/// GHD `offsetFromNow(value, unit)`: milliseconds since the epoch, `value`
/// `unit`s from now.
pub fn offset_from_now(value: i64, unit: Unit) -> i64 {
    offset_from(now_millis(), value, unit)
}

/// `Date.now()`.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}
