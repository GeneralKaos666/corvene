//! GHD `RelativeTime` (`ui/relative-time.tsx`): "just now", "5 minutes
//! ago", "2 hours ago", "3 days ago"…, from
//! `getRelativeTimeInfoFromDate` ([`relative_time_info`]). Like GHD's
//! component, which re-renders itself when its text is due to change, a
//! rendered relative time registers that moment and [`start_refresh`]
//! re-renders the windows then.
//!
//! Deviation (`106-calendar-relative-dates`): past a week, ages are counted
//! in weeks until two calendar months have passed, then in calendar months
//! and years (GHD `formatRelative` divides days by 30, so a commit on the 1st
//! is "last month" on the 31st).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use gpui_kit::App;

use crate::format::LocalTime;

static CALENDAR_DATES: AtomicBool = AtomicBool::new(false);

/// The earliest moment a relative time drawn since the last refresh is due
/// to change.
static NEXT_REFRESH: Mutex<Option<SystemTime>> = Mutex::new(None);

/// Mirror `106-calendar-relative-dates` (called whenever the flags change).
pub fn set_calendar_dates(on: bool) {
    CALENDAR_DATES.store(on, Ordering::Relaxed);
}

const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// GHD `RelativeTimeInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeTimeInfo {
    /// The tooltip text: the date and time in the user's formats.
    pub absolute_text: String,
    pub relative_text: String,
    /// When `relative_text` is due to change (GHD `duration`, the
    /// component's refresh timeout); `None` for an absolute date.
    pub duration: Option<Duration>,
}

/// GHD `getRelativeTimeInfoFromDate(then, onlyRelative)` with the clock's
/// `now` passed in. Without `only_relative` a date over a week old is
/// shown as the date alone.
pub fn relative_time_info(
    then: SystemTime,
    now: SystemTime,
    only_relative: bool,
) -> RelativeTimeInfo {
    // `formatDate(then, { dateStyle, timeStyle })`: the styles only apply
    // without formatting preferences, so the date and time formats
    let absolute_text = crate::format::format_date_time(then);
    let (future, duration) = match then.duration_since(now) {
        Ok(ahead) => (true, ahead),
        Err(e) => (false, e.duration()),
    };
    let info = |relative_text: String, duration: Option<Duration>| RelativeTimeInfo {
        absolute_text: absolute_text.clone(),
        relative_text,
        duration,
    };
    // more than a minute ahead: the date and time, rescheduled
    if future && duration > MINUTE {
        return info(absolute_text.clone(), Some(duration));
    }
    if duration < MINUTE {
        return info("just now".into(), Some(MINUTE - duration));
    }
    let relative_text = || format_relative(then, now, duration);
    if duration < HOUR {
        info(relative_text(), Some(MINUTE))
    } else if duration < DAY {
        info(relative_text(), Some(HOUR))
    } else if duration < 7 * DAY || only_relative {
        info(relative_text(), Some(6 * HOUR))
    } else {
        // more than a week ago, the date will suffice
        info(crate::format::format_date(then), None)
    }
}

/// GHD `formatRelative` of a past `age`, or the calendar wording of
/// `106-calendar-relative-dates`.
fn format_relative(then: SystemTime, now: SystemTime, age: Duration) -> String {
    let past_ms = age.as_millis() as f64;
    if CALENDAR_DATES.load(Ordering::Relaxed) {
        let (then, now) = (
            crate::format::local_time(then),
            crate::format::local_time(now),
        );
        return format_relative_calendar(past_ms, &then, &now);
    }
    format_relative_past(past_ms)
}

/// `RelativeTime` (`onlyRelative`) of `from` now.
pub fn relative(from: SystemTime) -> String {
    relative_at(from, SystemTime::now())
}

/// [`relative`] with the clock's `now` passed in. Registers when the text is
/// due to change for [`start_refresh`].
pub fn relative_at(from: SystemTime, now: SystemTime) -> String {
    // Settings › Appearance › "Prefer absolute dates over relative".
    if crate::format::prefer_absolute_dates() {
        return crate::format::format_date(from);
    }
    let info = relative_time_info(from, now, true);
    if let Some(duration) = info.duration
        && let Ok(mut next) = NEXT_REFRESH.lock()
    {
        let due = now + duration;
        if next.is_none_or(|n| due < n) {
            *next = Some(due);
        }
    }
    info.relative_text
}

/// How often [`start_refresh`] looks for a newly registered refresh.
const REFRESH_POLL: Duration = Duration::from_secs(15);

/// GHD `RelativeTime`'s timer for every relative time on screen: re-render
/// the windows once the earliest text drawn since the last refresh is due
/// to change. Call once at startup.
pub fn start_refresh(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            let now = SystemTime::now();
            let next = NEXT_REFRESH.lock().ok().and_then(|n| *n);
            match next {
                Some(due) if due <= now => {
                    if let Ok(mut n) = NEXT_REFRESH.lock() {
                        *n = None;
                    }
                    // the re-render registers the next change
                    cx.update(|cx| cx.refresh_windows());
                }
                Some(due) => {
                    let wait = due.duration_since(now).unwrap_or_default();
                    cx.background_executor().timer(wait.min(REFRESH_POLL)).await;
                }
                None => cx.background_executor().timer(REFRESH_POLL).await,
            }
        }
    })
    .detach();
}

/// `format_relative_past` up to a week; then weeks until two calendar months
/// have passed, calendar months below a year and calendar years after.
fn format_relative_calendar(ms: f64, then: &LocalTime, now: &LocalTime) -> String {
    let day = (ms / 86_400_000.0).floor();
    if day < 7.0 {
        return format_relative_past(ms);
    }
    let mut months = (now.year - then.year) * 12 + now.month as i32 - then.month as i32;
    let clock = |t: &LocalTime| (t.day, t.hour, t.minute, t.second);
    if clock(now) < clock(then) {
        months -= 1;
    }
    let months = months.max(0) as u64;
    let (n, unit) = if months < 2 {
        ((day / 7.0) as u64, "week")
    } else if months < 12 {
        (months, "month")
    } else {
        (months / 12, "year")
    };
    match n {
        1 => format!("last {unit}"),
        n => format!("{n} {unit}s ago"),
    }
}

/// GHD `formatRelative` (lib/format-relative.ts, after github/time-elements):
/// each unit is the rounded previous one, worded like
/// `Intl.RelativeTimeFormat('en-US', { numeric: 'auto' })`.
fn format_relative_past(ms: f64) -> String {
    let sec = (ms / 1000.0).round();
    let min = (sec / 60.0).round();
    let hr = (min / 60.0).round();
    let day = (hr / 24.0).round();
    let month = (day / 30.0).round();
    let year = (month / 12.0).round();
    let (n, unit) = if sec < 45.0 {
        (sec, "second")
    } else if min < 45.0 {
        (min, "minute")
    } else if hr < 24.0 {
        (hr, "hour")
    } else if day < 30.0 {
        (day, "day")
    } else if month < 18.0 {
        (month, "month")
    } else {
        (year, "year")
    };
    let n = n as u64;
    match (n, unit) {
        (0, "second") => "now".to_string(),
        (0, other) => format!("this {other}"),
        (1, "day") => "yesterday".to_string(),
        (1, "month") => "last month".to_string(),
        (1, "year") => "last year".to_string(),
        (1, other) => format!("1 {other} ago"),
        (n, other) => format!("{n} {other}s ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn info_schedules_the_next_change() {
        let now = SystemTime::now();
        let info = relative_time_info(now - Duration::from_secs(44), now, true);
        assert_eq!(info.relative_text, "just now");
        assert_eq!(info.duration, Some(Duration::from_secs(16)));
        let info = relative_time_info(now - Duration::from_secs(3 * 3600), now, true);
        assert_eq!(info.relative_text, "3 hours ago");
        assert_eq!(info.duration, Some(HOUR));
        let old = now - 8 * DAY;
        let info = relative_time_info(old, now, false);
        assert_eq!(info.relative_text, crate::format::format_date(old));
        assert_eq!(info.duration, None);
    }

    #[test]
    fn buckets() {
        let now = SystemTime::now();
        assert_eq!(relative(now), "just now");
        assert_eq!(relative(now - Duration::from_secs(90)), "2 minutes ago");
        assert_eq!(relative(now - Duration::from_secs(7200)), "2 hours ago");
        assert_eq!(
            relative(now - Duration::from_secs(86_400 * 3)),
            "3 days ago"
        );
    }

    fn at(year: i32, month: u32, day: u32) -> LocalTime {
        LocalTime {
            year,
            month,
            day,
            hour: 12,
            minute: 0,
            second: 0,
        }
    }

    #[test]
    fn calendar_weeks_months_and_years() {
        let day = 86_400_000.0;
        let now = at(2026, 7, 31);
        let cal = |days: f64, then: LocalTime| format_relative_calendar(days * day, &then, &now);
        assert_eq!(cal(3.0, at(2026, 7, 28)), "3 days ago");
        assert_eq!(cal(8.0, at(2026, 7, 23)), "last week");
        // #20830: the 1st is 4 weeks ago on the 31st, not last month
        assert_eq!(cal(30.0, at(2026, 7, 1)), "4 weeks ago");
        assert_eq!(cal(35.0, at(2026, 6, 26)), "5 weeks ago");
        assert_eq!(cal(60.0, at(2026, 6, 1)), "8 weeks ago");
        assert_eq!(cal(61.0, at(2026, 5, 31)), "2 months ago");
        assert_eq!(cal(200.0, at(2026, 1, 12)), "6 months ago");
        assert_eq!(cal(365.0, at(2025, 7, 31)), "last year");
        assert_eq!(cal(800.0, at(2024, 5, 23)), "2 years ago");
    }

    #[test]
    fn rounds_and_words_like_intl_numeric_auto() {
        let day = 86_400_000.0;
        assert_eq!(format_relative_past(27.6 * day), "28 days ago");
        assert_eq!(format_relative_past(day), "yesterday");
        assert_eq!(format_relative_past(42.0 * day), "last month");
        assert_eq!(format_relative_past(58.0 * day), "2 months ago");
        assert_eq!(format_relative_past(44.0 * 60_000.0), "44 minutes ago");
        assert_eq!(format_relative_past(50.0 * 60_000.0), "1 hour ago");
        assert_eq!(format_relative_past(600.0 * day), "2 years ago");
    }
}
