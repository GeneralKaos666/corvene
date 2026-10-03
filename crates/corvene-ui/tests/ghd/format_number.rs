//! Port of GitHub Desktop's `app/test/unit/format-number-test.ts`.
//!
//! - `formatNumber(value, fmt)` (`lib/format-number.ts`) is
//!   [`corvene_ui::format::format_number_with`]`(value, thousands, decimal)`,
//!   the formatter behind Settings › Appearance › Formatting and
//!   `format::format_count`. GitHub Desktop's `INumberFormat`
//!   (`models/formatting-preferences.ts`) is the [`INumberFormat`] below;
//!   [`format_number`] hands its two separators to `format_number_with`.
//!   `format_number_with` has no `maximumFractionDigits`, so the cases that
//!   set it reach a stand-in and are ignored.
//! - `formatCompactNumber(value, fmt)` (the toolbar push / pull counts and
//!   the changed-files badge in GitHub Desktop) has no Corvene equivalent:
//!   the toolbar prints the plain count. Its cases call the stand-in
//!   [`format_compact_number`] and are ignored until it exists.

// GitHub Desktop's `3.14159` is an input to format, not an approximation of π.
#![allow(clippy::approx_constant)]

use corvene_ui::format::format_number_with;

/// GitHub Desktop's `INumberFormat`.
#[derive(Clone, Copy)]
struct INumberFormat {
    thousands_separator: &'static str,
    decimal_separator: &'static str,
    maximum_fraction_digits: Option<usize>,
}

// Standard number formats for testing
const COMMA_THOUSANDS_DOT_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: ",",
    decimal_separator: ".",
    maximum_fraction_digits: None,
};

const DOT_THOUSANDS_COMMA_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: ".",
    decimal_separator: ",",
    maximum_fraction_digits: None,
};

const SPACE_THOUSANDS_DOT_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: " ",
    decimal_separator: ".",
    maximum_fraction_digits: None,
};

const SPACE_THOUSANDS_COMMA_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: " ",
    decimal_separator: ",",
    maximum_fraction_digits: None,
};

const NO_THOUSANDS_DOT_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: "",
    decimal_separator: ".",
    maximum_fraction_digits: None,
};

const NO_THOUSANDS_COMMA_DECIMAL: INumberFormat = INumberFormat {
    thousands_separator: "",
    decimal_separator: ",",
    maximum_fraction_digits: None,
};

/// GitHub Desktop's `formatNumber(value, fmt)`: Corvene's
/// `format_number_with` with the format's separators. A format with
/// `maximumFractionDigits` reaches a stand-in: replace it with the Corvene
/// call once `format_number_with` (or a sibling) takes the maximum number
/// of fraction digits, and remove the `#[ignore]`s.
fn format_number(value: f64, fmt: &INumberFormat) -> String {
    match fmt.maximum_fraction_digits {
        None => format_number_with(value, fmt.thousands_separator, fmt.decimal_separator),
        Some(_) => unimplemented!("format_number_with has no maximumFractionDigits"),
    }
}

/// GitHub Desktop's `ICompactFormatOptions` (`lib/format-number.ts`).
#[derive(Clone, Copy, Default)]
#[allow(dead_code)] // read once the stand-in below is replaced
struct ICompactFormatOptions {
    /// Number of decimal places to display
    decimals: Option<u32>,
    /// 1000 (k, m, b, t) or 1024 (KiB, MiB, GiB)
    base: Option<u32>,
    /// Unit suffixes, `['', 'k', 'm', 'b', 't']` for base 1000
    units: Option<&'static [&'static str]>,
    /// Between the number and the unit, `''` by default
    unit_separator: Option<&'static str>,
    number_format: Option<INumberFormat>,
}

/// Stand-in for GitHub Desktop's `formatCompactNumber(value, fmt)`
/// (`lib/format-number.ts`): `1.2k`, `10m`, `1,000t`. Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`s.
fn format_compact_number(_value: f64, _fmt: &ICompactFormatOptions) -> String {
    unimplemented!("Corvene has no formatCompactNumber")
}

/// `{ numberFormat: fmt }`
fn with_format(fmt: INumberFormat) -> ICompactFormatOptions {
    ICompactFormatOptions {
        number_format: Some(fmt),
        ..Default::default()
    }
}

/// `{ numberFormat: fmt, decimals }`
fn with_decimals(fmt: INumberFormat, decimals: u32) -> ICompactFormatOptions {
    ICompactFormatOptions {
        number_format: Some(fmt),
        decimals: Some(decimals),
        ..Default::default()
    }
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats small integers without thousands separator
#[test]
fn formats_small_integers_without_thousands_separator() {
    assert_eq!(format_number(0., &COMMA_THOUSANDS_DOT_DECIMAL), "0");
    assert_eq!(format_number(1., &COMMA_THOUSANDS_DOT_DECIMAL), "1");
    assert_eq!(format_number(42., &COMMA_THOUSANDS_DOT_DECIMAL), "42");
    assert_eq!(format_number(999., &COMMA_THOUSANDS_DOT_DECIMAL), "999");
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats thousands with comma separator
#[test]
fn formats_thousands_with_comma_separator() {
    assert_eq!(format_number(1000., &COMMA_THOUSANDS_DOT_DECIMAL), "1,000");
    assert_eq!(
        format_number(12345., &COMMA_THOUSANDS_DOT_DECIMAL),
        "12,345"
    );
    assert_eq!(
        format_number(999999., &COMMA_THOUSANDS_DOT_DECIMAL),
        "999,999"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats millions with multiple separators
#[test]
fn formats_millions_with_multiple_separators() {
    assert_eq!(
        format_number(1000000., &COMMA_THOUSANDS_DOT_DECIMAL),
        "1,000,000"
    );
    assert_eq!(
        format_number(1234567890., &COMMA_THOUSANDS_DOT_DECIMAL),
        "1,234,567,890"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats thousands with dot separator (European style)
#[test]
fn formats_thousands_with_dot_separator_european_style() {
    assert_eq!(format_number(1000., &DOT_THOUSANDS_COMMA_DECIMAL), "1.000");
    assert_eq!(
        format_number(12345., &DOT_THOUSANDS_COMMA_DECIMAL),
        "12.345"
    );
    assert_eq!(
        format_number(1234567., &DOT_THOUSANDS_COMMA_DECIMAL),
        "1.234.567"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats thousands with space separator
#[test]
fn formats_thousands_with_space_separator() {
    assert_eq!(format_number(1000., &SPACE_THOUSANDS_DOT_DECIMAL), "1 000");
    assert_eq!(
        format_number(12345., &SPACE_THOUSANDS_DOT_DECIMAL),
        "12 345"
    );
    assert_eq!(
        format_number(1234567., &SPACE_THOUSANDS_DOT_DECIMAL),
        "1 234 567"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › integers › formats without thousands separator when configured
#[test]
fn formats_without_thousands_separator_when_configured() {
    assert_eq!(format_number(1000., &NO_THOUSANDS_DOT_DECIMAL), "1000");
    assert_eq!(format_number(12345., &NO_THOUSANDS_DOT_DECIMAL), "12345");
    assert_eq!(
        format_number(1234567., &NO_THOUSANDS_DOT_DECIMAL),
        "1234567"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › formats decimals with dot separator
#[test]
fn formats_decimals_with_dot_separator() {
    assert_eq!(format_number(1.5, &COMMA_THOUSANDS_DOT_DECIMAL), "1.5");
    assert_eq!(
        format_number(3.14159, &COMMA_THOUSANDS_DOT_DECIMAL),
        "3.14159"
    );
    assert_eq!(format_number(0.123, &COMMA_THOUSANDS_DOT_DECIMAL), "0.123");
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › formats decimals with comma separator (European style)
#[test]
fn formats_decimals_with_comma_separator_european_style() {
    assert_eq!(format_number(1.5, &DOT_THOUSANDS_COMMA_DECIMAL), "1,5");
    assert_eq!(
        format_number(3.14159, &DOT_THOUSANDS_COMMA_DECIMAL),
        "3,14159"
    );
    assert_eq!(format_number(0.123, &DOT_THOUSANDS_COMMA_DECIMAL), "0,123");
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › formats large numbers with decimals
#[test]
fn formats_large_numbers_with_decimals() {
    assert_eq!(
        format_number(1234567.89, &COMMA_THOUSANDS_DOT_DECIMAL),
        "1,234,567.89"
    );
    assert_eq!(
        format_number(1234567.89, &DOT_THOUSANDS_COMMA_DECIMAL),
        "1.234.567,89"
    );
    assert_eq!(
        format_number(1234567.89, &SPACE_THOUSANDS_DOT_DECIMAL),
        "1 234 567.89"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › truncates decimals to the maximum fraction digits
#[test]
#[ignore = "ghd: missing: format_number_with has no maximumFractionDigits (GHD formatNumber, lib/format-number.ts)"]
fn truncates_decimals_to_the_maximum_fraction_digits() {
    assert_eq!(
        format_number(
            3.14159,
            &INumberFormat {
                maximum_fraction_digits: Some(2),
                ..COMMA_THOUSANDS_DOT_DECIMAL
            }
        ),
        "3.14"
    );
    assert_eq!(
        format_number(
            3.199,
            &INumberFormat {
                maximum_fraction_digits: Some(2),
                ..DOT_THOUSANDS_COMMA_DECIMAL
            }
        ),
        "3,19"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › omits the decimal separator when maximum fraction digits is zero
#[test]
#[ignore = "ghd: missing: format_number_with has no maximumFractionDigits (GHD formatNumber, lib/format-number.ts)"]
fn omits_the_decimal_separator_when_maximum_fraction_digits_is_zero() {
    assert_eq!(
        format_number(
            1234.5,
            &INumberFormat {
                maximum_fraction_digits: Some(0),
                ..COMMA_THOUSANDS_DOT_DECIMAL
            }
        ),
        "1,234"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › decimals › leaves integers unchanged when maximum fraction digits is set
#[test]
#[ignore = "ghd: missing: format_number_with has no maximumFractionDigits (GHD formatNumber, lib/format-number.ts)"]
fn leaves_integers_unchanged_when_maximum_fraction_digits_is_set() {
    assert_eq!(
        format_number(
            1234.,
            &INumberFormat {
                maximum_fraction_digits: Some(2),
                ..COMMA_THOUSANDS_DOT_DECIMAL
            }
        ),
        "1,234"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › negative numbers › formats negative integers
#[test]
fn formats_negative_integers() {
    assert_eq!(format_number(-1., &COMMA_THOUSANDS_DOT_DECIMAL), "-1");
    assert_eq!(
        format_number(-1000., &COMMA_THOUSANDS_DOT_DECIMAL),
        "-1,000"
    );
    assert_eq!(
        format_number(-1234567., &COMMA_THOUSANDS_DOT_DECIMAL),
        "-1,234,567"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › negative numbers › formats negative decimals
#[test]
fn formats_negative_decimals() {
    assert_eq!(format_number(-1.5, &COMMA_THOUSANDS_DOT_DECIMAL), "-1.5");
    assert_eq!(
        format_number(-1234.56, &COMMA_THOUSANDS_DOT_DECIMAL),
        "-1,234.56"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › edge cases › handles Infinity
#[test]
#[ignore = "ghd: bug: format_number_with(f64::INFINITY) gives inf and -inf (Rust Display), GHD String(value) gives Infinity and -Infinity"]
fn format_number_handles_infinity() {
    assert_eq!(
        format_number(f64::INFINITY, &COMMA_THOUSANDS_DOT_DECIMAL),
        "Infinity"
    );
    assert_eq!(
        format_number(f64::NEG_INFINITY, &COMMA_THOUSANDS_DOT_DECIMAL),
        "-Infinity"
    );
}

// GHD: unit/format-number-test.ts › formatNumber › edge cases › handles NaN
#[test]
fn format_number_handles_nan() {
    assert_eq!(format_number(f64::NAN, &COMMA_THOUSANDS_DOT_DECIMAL), "NaN");
}

// GHD: unit/format-number-test.ts › formatNumber › edge cases › handles very small decimals
#[test]
fn handles_very_small_decimals() {
    assert_eq!(format_number(0.001, &COMMA_THOUSANDS_DOT_DECIMAL), "0.001");
    assert_eq!(
        format_number(0.000001, &COMMA_THOUSANDS_DOT_DECIMAL),
        "0.000001"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › small numbers (< 1000) › formats small numbers without compaction
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_small_numbers_without_compaction() {
    assert_eq!(
        format_compact_number(0., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "0"
    );
    assert_eq!(
        format_compact_number(1., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1"
    );
    assert_eq!(
        format_compact_number(42., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "42"
    );
    assert_eq!(
        format_compact_number(999., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "999"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › small numbers (< 1000) › formats small decimals without compaction
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_small_decimals_without_compaction() {
    assert_eq!(
        format_compact_number(1.5, &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.5"
    );
    assert_eq!(
        format_compact_number(123.45, &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "123.45"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › thousands (k) › formats thousands with k suffix
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_thousands_with_k_suffix() {
    assert_eq!(
        format_compact_number(1000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1k"
    );
    assert_eq!(
        format_compact_number(1500., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.5k"
    );
    assert_eq!(
        format_compact_number(9999., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "10k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › thousands (k) › shows one decimal for values under 10k
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_one_decimal_for_values_under_10k() {
    assert_eq!(
        format_compact_number(1234., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.2k"
    );
    assert_eq!(
        format_compact_number(5678., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "5.7k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › thousands (k) › shows no decimals for values 10k and above
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_no_decimals_for_values_10k_and_above() {
    assert_eq!(
        format_compact_number(10000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "10k"
    );
    assert_eq!(
        format_compact_number(12345., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "12k"
    );
    assert_eq!(
        format_compact_number(99999., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "100k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › thousands (k) › uses configured decimal separator
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn uses_configured_decimal_separator() {
    assert_eq!(
        format_compact_number(1234., &with_format(DOT_THOUSANDS_COMMA_DECIMAL)),
        "1,2k"
    );
    assert_eq!(
        format_compact_number(5678., &with_format(SPACE_THOUSANDS_COMMA_DECIMAL)),
        "5,7k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › millions (m) › formats millions with m suffix
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_millions_with_m_suffix() {
    assert_eq!(
        format_compact_number(1000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1m"
    );
    assert_eq!(
        format_compact_number(1500000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.5m"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › millions (m) › shows one decimal for values under 10m
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_one_decimal_for_values_under_10m() {
    assert_eq!(
        format_compact_number(1234567., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.2m"
    );
    assert_eq!(
        format_compact_number(9876543., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "9.9m"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › millions (m) › shows no decimals for values 10m and above
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_no_decimals_for_values_10m_and_above() {
    assert_eq!(
        format_compact_number(10000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "10m"
    );
    assert_eq!(
        format_compact_number(99999999., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "100m"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › billions (b) › formats billions with b suffix
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_billions_with_b_suffix() {
    assert_eq!(
        format_compact_number(1000000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1b"
    );
    assert_eq!(
        format_compact_number(1500000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.5b"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › billions (b) › shows one decimal for values under 10b
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_one_decimal_for_values_under_10b() {
    assert_eq!(
        format_compact_number(1234567890., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.2b"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › billions (b) › shows no decimals for values 10b and above
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn shows_no_decimals_for_values_10b_and_above() {
    assert_eq!(
        format_compact_number(10000000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "10b"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › trillions (t) › formats trillions with t suffix
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn formats_trillions_with_t_suffix() {
    assert_eq!(
        format_compact_number(1000000000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1t"
    );
    assert_eq!(
        format_compact_number(1500000000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1.5t"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › trillions (t) › caps at trillion for extremely large numbers
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn caps_at_trillion_for_extremely_large_numbers() {
    // Quadrillions and beyond still use 't' suffix
    assert_eq!(
        format_compact_number(1000000000000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "1,000t"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › edge cases › handles Infinity
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn format_compact_number_handles_infinity() {
    assert_eq!(
        format_compact_number(f64::INFINITY, &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "Infinity"
    );
    assert_eq!(
        format_compact_number(f64::NEG_INFINITY, &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "-Infinity"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › edge cases › handles NaN
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn format_compact_number_handles_nan() {
    assert_eq!(
        format_compact_number(f64::NAN, &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "NaN"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › edge cases › handles negative large numbers
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn handles_negative_large_numbers() {
    assert_eq!(
        format_compact_number(-1234., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "-1.2k"
    );
    assert_eq!(
        format_compact_number(-1000000., &with_format(COMMA_THOUSANDS_DOT_DECIMAL)),
        "-1m"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › explicit decimals › uses explicit decimals when provided
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn uses_explicit_decimals_when_provided() {
    // By default, 12345 would show '12k' (0 decimals for >= 10)
    // With explicit decimals: 2, it should show '12.35k'
    assert_eq!(
        format_compact_number(12345., &with_decimals(COMMA_THOUSANDS_DOT_DECIMAL, 2)),
        "12.35k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › explicit decimals › respects explicit decimals of 0
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn respects_explicit_decimals_of_0() {
    // By default, 1234 would show '1.2k' (1 decimal for < 10)
    // With explicit decimals: 0, it should show '1k'
    assert_eq!(
        format_compact_number(1234., &with_decimals(COMMA_THOUSANDS_DOT_DECIMAL, 0)),
        "1k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › explicit decimals › works with explicit decimals across magnitude boundaries
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn works_with_explicit_decimals_across_magnitude_boundaries() {
    assert_eq!(
        format_compact_number(1234567., &with_decimals(COMMA_THOUSANDS_DOT_DECIMAL, 3)),
        "1.235m"
    );
    assert_eq!(
        format_compact_number(1234567890., &with_decimals(COMMA_THOUSANDS_DOT_DECIMAL, 2)),
        "1.23b"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › explicit decimals › uses configured decimal separator with explicit decimals
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn uses_configured_decimal_separator_with_explicit_decimals() {
    assert_eq!(
        format_compact_number(12345., &with_decimals(DOT_THOUSANDS_COMMA_DECIMAL, 2)),
        "12,35k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › all format configurations › works with space thousands and dot decimal
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn works_with_space_thousands_and_dot_decimal() {
    assert_eq!(
        format_compact_number(1234., &with_format(SPACE_THOUSANDS_DOT_DECIMAL)),
        "1.2k"
    );
    assert_eq!(
        format_compact_number(1000000000000000., &with_format(SPACE_THOUSANDS_DOT_DECIMAL)),
        "1 000t"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › all format configurations › works with space thousands and comma decimal
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn works_with_space_thousands_and_comma_decimal() {
    assert_eq!(
        format_compact_number(1234., &with_format(SPACE_THOUSANDS_COMMA_DECIMAL)),
        "1,2k"
    );
}

// GHD: unit/format-number-test.ts › formatCompactNumber › all format configurations › works with no thousands separator
#[test]
#[ignore = "ghd: missing: Corvene has no formatCompactNumber (lib/format-number.ts)"]
fn works_with_no_thousands_separator() {
    assert_eq!(
        format_compact_number(1234., &with_format(NO_THOUSANDS_DOT_DECIMAL)),
        "1.2k"
    );
    assert_eq!(
        format_compact_number(1234., &with_format(NO_THOUSANDS_COMMA_DECIMAL)),
        "1,2k"
    );
    assert_eq!(
        format_compact_number(1000000000000000., &with_format(NO_THOUSANDS_DOT_DECIMAL)),
        "1000t"
    );
}
