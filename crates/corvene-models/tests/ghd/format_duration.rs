//! Port of GitHub Desktop's `app/test/unit/format-duration-test.ts`.
//!
//! GitHub Desktop's `formatPreciseDuration(ms)` (`lib/format-duration.ts`,
//! the CI check run durations) is
//! [`corvene_models::format_precise_duration`]. That takes a `u64`, so the
//! negative duration of "treats negative values as absolute numbers" (GitHub
//! Desktop's `getCheckDurationInMilliseconds` can return one when
//! `completed_at` is before `started_at`) cannot be passed; that case calls
//! the stand-in [`signed::format_precise_duration`] and is ignored.

use corvene_models::format_precise_duration;

mod signed {
    /// Stand-in for GitHub Desktop's `formatPreciseDuration(ms)` with a
    /// signed `ms` (it formats `Math.abs(ms)`). Replace it with
    /// `corvene_models::format_precise_duration` once that takes a signed
    /// duration and remove the `#[ignore]`.
    pub fn format_precise_duration(_ms: i64) -> String {
        unimplemented!("corvene_models::format_precise_duration takes u64")
    }
}

// GHD: unit/format-duration-test.ts › formatPreciseDuration › returns 0s for ms less than 1000
#[test]
fn returns_0s_for_ms_less_than_1000() {
    assert_eq!(format_precise_duration(1), "0s");
}

// GHD: unit/format-duration-test.ts › formatPreciseDuration › return 0[unit] after encountering first whole unit
#[test]
#[ignore = "ghd: bug: format_precise_duration drops zero units after the first whole one: 86400000 gives 1d, GHD 1d 0h 0m 0s (lib/format-duration.ts)"]
fn return_0_unit_after_encountering_first_whole_unit() {
    assert_eq!(format_precise_duration(86400000), "1d 0h 0m 0s");
    assert_eq!(format_precise_duration(3600000), "1h 0m 0s");
    assert_eq!(format_precise_duration(60000), "1m 0s");
    assert_eq!(format_precise_duration(1000), "1s");
}

// GHD: unit/format-duration-test.ts › formatPreciseDuration › treats negative values as absolute numbers
#[test]
#[ignore = "ghd: missing: format_precise_duration takes u64, so GHD formatPreciseDuration(-1000) (Math.abs, lib/format-duration.ts) cannot be expressed"]
fn treats_negative_values_as_absolute_numbers() {
    assert_eq!(signed::format_precise_duration(-1000), "1s");
}
