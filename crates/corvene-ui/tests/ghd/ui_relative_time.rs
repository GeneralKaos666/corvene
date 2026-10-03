//! Port of GitHub Desktop's `app/test/unit/ui/relative-time-test.tsx`.
//!
//! GitHub Desktop's `RelativeTime` component (`ui/relative-time.tsx`) shows
//! `getRelativeTimeInfoFromDate(date, onlyRelative).relativeText` and
//! schedules a re-render after its `duration`. The tests fix the clock at
//! 2026-03-26T12:00:00Z (`enableTestTimers(['Date', 'setTimeout'], now)`).
//!
//! Corvene's counterpart is `corvene_ui::relative_time::relative(date)`,
//! which the history, branch and undo-commit views call on every render,
//! built on `relative_time::relative_time_info(then, now, only_relative)`
//! (GitHub Desktop's `getRelativeTimeInfoFromDate` with the clock's now
//! passed in). So:
//!
//! - the first three cases, whose expected text depends only on how far the
//!   date is from now, call `relative` with the real clock's now minus the
//!   case's offset (the text is the same at any instant); `tooltip` (the
//!   `TooltippedContent` wrapper versus a plain `<span>`) is React DOM and
//!   not ported, and React's re-render on a new `date` prop is a second
//!   `relative` call, as in Corvene's views;
//! - the refresh and `onlyRelative: false` cases need the fixed clock and
//!   the refresh schedule, so they call [`get_relative_time_info_from_date`].
//!
//! The absolute date GitHub Desktop expects (`formatDate(then, { date: true,
//! time: false })`, the user's date format) is `corvene_ui::format::format_date`.
//! Flag `106-calendar-relative-dates` only changes ages past a week and is
//! not involved (the offsets are under a day).

use std::time::{Duration, SystemTime};

use corvene_test_support::date_parse;
use corvene_ui::format::format_date;
use corvene_ui::relative_time::{RelativeTimeInfo, relative, relative_time_info};

const SECOND: Duration = Duration::from_secs(1);
const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// `Date.parse('2026-03-26T12:00:00.000Z')`.
fn now() -> SystemTime {
    date_parse("2026-03-26T12:00:00.000Z")
}

/// GitHub Desktop's `getRelativeTimeInfoFromDate(then, onlyRelative)`
/// (`ui/relative-time.tsx`) with the clock's `now` passed in (GitHub Desktop
/// reads the mocked `Date.now()`).
fn get_relative_time_info_from_date(
    then: SystemTime,
    now: SystemTime,
    only_relative: bool,
) -> RelativeTimeInfo {
    relative_time_info(then, now, only_relative)
}

// GHD: unit/ui/relative-time-test.tsx › RelativeTime › renders recent relative text without a tooltip wrapper when disabled
#[test]
fn renders_recent_relative_text_without_a_tooltip_wrapper_when_disabled() {
    let text = relative(SystemTime::now() - 30 * SECOND);

    assert_eq!(text, "just now");
}

// GHD: unit/ui/relative-time-test.tsx › RelativeTime › renders recent relative text with the default tooltip-enabled path
#[test]
fn renders_recent_relative_text_with_the_default_tooltip_enabled_path() {
    let text = relative(SystemTime::now() - 30 * SECOND);

    assert_eq!(text, "just now");
}

// GHD: unit/ui/relative-time-test.tsx › RelativeTime › updates its rendered text when the date prop changes
#[test]
fn updates_its_rendered_text_when_the_date_prop_changes() {
    assert_eq!(relative(SystemTime::now() - 2 * MINUTE), "2 minutes ago");

    // `view.rerender(<RelativeTime date={now - 2 hours} />)`
    assert_eq!(relative(SystemTime::now() - 2 * HOUR), "2 hours ago");
}

// GHD: unit/ui/relative-time-test.tsx › RelativeTime › refreshes once the scheduled timeout elapses
#[test]
fn refreshes_once_the_scheduled_timeout_elapses() {
    let then = now() - 44 * SECOND;
    let info = get_relative_time_info_from_date(then, now(), true);

    assert_eq!(info.relative_text, "just now");

    // `advanceTimersBy(16 * 1000)`: the refresh scheduled `duration` after
    // the render fires by then and renders with the clock at that moment
    let refresh = info.duration.expect("RelativeTime schedules a refresh");
    assert!(refresh <= 16 * SECOND);
    let refreshed = get_relative_time_info_from_date(then, now() + refresh, true);

    assert_eq!(refreshed.relative_text, "1 minute ago");
}

// GHD: unit/ui/relative-time-test.tsx › RelativeTime › renders an absolute date for older timestamps when onlyRelative is false
#[test]
fn renders_an_absolute_date_for_older_timestamps_when_only_relative_is_false() {
    let then = now() - 8 * DAY;

    let info = get_relative_time_info_from_date(then, now(), false);

    let absolute_date = format_date(then);
    assert_eq!(info.relative_text, absolute_date);
}
