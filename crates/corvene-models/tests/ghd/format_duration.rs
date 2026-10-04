//! Port of GitHub Desktop's `app/test/unit/format-duration-test.ts`.
//!
//! GitHub Desktop's `formatPreciseDuration(ms)` (`lib/format-duration.ts`,
//! the CI check run durations) is
//! [`corvene_models::format_precise_duration`], which takes a signed `ms`
//! (GitHub Desktop's `getCheckDurationInMilliseconds` is negative when
//! `completed_at` is before `started_at`).

use corvene_models::format_precise_duration;

// GHD: unit/format-duration-test.ts › formatPreciseDuration › returns 0s for ms less than 1000
#[test]
fn returns_0s_for_ms_less_than_1000() {
    assert_eq!(format_precise_duration(1), "0s");
}

// GHD: unit/format-duration-test.ts › formatPreciseDuration › return 0[unit] after encountering first whole unit
#[test]
fn return_0_unit_after_encountering_first_whole_unit() {
    assert_eq!(format_precise_duration(86400000), "1d 0h 0m 0s");
    assert_eq!(format_precise_duration(3600000), "1h 0m 0s");
    assert_eq!(format_precise_duration(60000), "1m 0s");
    assert_eq!(format_precise_duration(1000), "1s");
}

// GHD: unit/format-duration-test.ts › formatPreciseDuration › treats negative values as absolute numbers
#[test]
fn treats_negative_values_as_absolute_numbers() {
    assert_eq!(format_precise_duration(-1000), "1s");
}
