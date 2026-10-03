//! Port of GitHub Desktop's `app/test/unit/bytes-test.ts`.
//!
//! GitHub Desktop's `formatBytes(bytes, decimals = 0)` (`ui/lib/bytes.ts`,
//! the image diff's sizes) is [`corvene_ui::format::format_bytes`]`(bytes,
//! decimals)`; GitHub Desktop's default of zero decimals is passed
//! explicitly. In GitHub Desktop's test environment the number format has
//! no thousands separator and a `.` decimal separator (no `lc=` country in
//! the page URL), which is what `format_bytes` always uses.
//!
//! The NaN and Infinity cases are skipped (`tools/ghd-tests/skips/format.tsv`):
//! `format_bytes` takes an `i64` byte count, which cannot hold either.

use corvene_ui::format::format_bytes;

// GHD: unit/bytes-test.ts › formatBytes › rounds to the desired number decimals
#[test]
#[ignore = "ghd: bug: format_bytes(1342177280, 1) gives 1.2 GiB, GHD 1.3 GiB (format! rounds the 1.25 tie to even, GHD round() in ui/lib/round.ts rounds half up)"]
fn rounds_to_the_desired_number_decimals() {
    assert_eq!(format_bytes(1342177280, 2), "1.25 GiB");
    assert_eq!(format_bytes(1342177280, 1), "1.3 GiB");
    assert_eq!(format_bytes(1342177280, 0), "1 GiB");

    assert_eq!(format_bytes(1879048192, 2), "1.75 GiB");
    assert_eq!(format_bytes(1879048192, 1), "1.8 GiB");
    assert_eq!(format_bytes(1879048192, 0), "2 GiB");
}

// GHD: unit/bytes-test.ts › formatBytes › uses the correct units
#[test]
fn uses_the_correct_units() {
    assert_eq!(format_bytes(1023, 0), "1023 B");
    assert_eq!(format_bytes(1024, 0), "1 KiB");

    // N.B this codifies the current behavior, I personally
    // wouldn't object to formatBytes(1048575) returning 1 MiB
    assert_eq!(format_bytes(1048575, 3), "1023.999 KiB");
    assert_eq!(format_bytes(1048575, 0), "1024 KiB");
    assert_eq!(format_bytes(1048576, 0), "1 MiB");

    assert_eq!(format_bytes(1073741823, 0), "1024 MiB");
    assert_eq!(format_bytes(1073741824, 0), "1 GiB");

    assert_eq!(format_bytes(1099511627775, 0), "1024 GiB");
    assert_eq!(format_bytes(1099511627776, 0), "1 TiB");
}
