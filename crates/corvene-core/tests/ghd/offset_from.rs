//! Port of GitHub Desktop's `app/test/unit/offset-from-test.ts`.
//!
//! GitHub Desktop's `offsetFrom` / `offsetFromNow` (`lib/offset-from.ts`)
//! shift a time by a number of seconds, minutes, hours, days or (365-day)
//! years; its stores use them for thresholds such as "fetched more than an
//! hour ago". Corvene's are `corvene_core::offset_from::{offset_from,
//! offset_from_now}`.
//!
//! JavaScript's two kinds of time map to milliseconds since the epoch as an
//! `i64` (`number`, as `Date.now()` returns) and `SystemTime` (`Date`);
//! GitHub Desktop's overloads, which return the kind they are given, become
//! one function generic over both (`corvene_core::offset_from::Dateish`).

use std::any::Any;
use std::time::{SystemTime, UNIX_EPOCH};

use corvene_core::offset_from::{Unit, offset_from, offset_from_now};
use corvene_test_support::{date_parse, to_iso_string};

/// JavaScript's `Date.now()`.
fn date_now() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

/// JavaScript's `new Date(isoString)`.
fn new_date(iso: &str) -> SystemTime {
    date_parse(iso)
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › offsets by seconds
#[test]
fn offsets_by_seconds() {
    let base: i64 = 1000000;
    let result = offset_from(base, 5, Unit::Seconds);
    assert_eq!(result, 1005000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › offsets by minutes
#[test]
fn offsets_by_minutes() {
    let base: i64 = 0;
    let result = offset_from(base, 2, Unit::Minutes);
    assert_eq!(result, 120000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › offsets by hours
#[test]
fn offsets_by_hours() {
    let base: i64 = 0;
    let result = offset_from(base, 1, Unit::Hour);
    assert_eq!(result, 3600000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › offsets by days
#[test]
fn offsets_by_days() {
    let base: i64 = 0;
    let result = offset_from(base, 1, Unit::Day);
    assert_eq!(result, 86400000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › offsets by years
#[test]
fn offsets_by_years() {
    let base: i64 = 0;
    let result = offset_from(base, 1, Unit::Year);
    assert_eq!(result, 31536000000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › handles negative offsets
#[test]
fn handles_negative_offsets() {
    let base: i64 = 100000;
    let result = offset_from(base, -1, Unit::Seconds);
    assert_eq!(result, 99000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with number input › returns a number when given a number
#[test]
fn returns_a_number_when_given_a_number() {
    let result = offset_from(0_i64, 1, Unit::Second);
    // `typeof result === 'number'`
    assert!((&result as &dyn Any).is::<i64>());
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with Date input › returns a Date when given a Date
#[test]
fn returns_a_date_when_given_a_date() {
    // `new Date(2025, 0, 1)` is local midnight; only the kind of the result
    // is checked, so UTC midnight stands in for it.
    let base = new_date("2025-01-01T00:00:00Z");
    let result = offset_from(base, 1, Unit::Day);
    // `result instanceof Date`
    assert!((&result as &dyn Any).is::<SystemTime>());
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with Date input › offsets a Date by the correct amount
#[test]
fn offsets_a_date_by_the_correct_amount() {
    let base = new_date("2025-01-01T00:00:00Z");
    let result = offset_from(base, 1, Unit::Day);
    assert_eq!(to_iso_string(result), "2025-01-02T00:00:00.000Z");
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFrom with Date input › offsets a Date by negative amount
#[test]
fn offsets_a_date_by_negative_amount() {
    let base = new_date("2025-01-02T00:00:00Z");
    let result = offset_from(base, -1, Unit::Day);
    assert_eq!(to_iso_string(result), "2025-01-01T00:00:00.000Z");
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFromNow › returns a timestamp close to now plus the offset
#[test]
fn returns_a_timestamp_close_to_now_plus_the_offset() {
    let before = date_now();
    let result = offset_from_now(1, Unit::Second);
    let after = date_now();

    // result should be approximately now + 1000ms
    assert!(result >= before + 1000);
    assert!(result <= after + 1000);
}

// GHD: unit/offset-from-test.ts › offset-from › offsetFromNow › returns a past timestamp for negative offsets
#[test]
fn returns_a_past_timestamp_for_negative_offsets() {
    let before = date_now();
    let result = offset_from_now(-1, Unit::Hour);

    assert!(result < before);
    assert!(result >= before - 3600000 - 100); // small tolerance
}
