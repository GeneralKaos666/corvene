//! Port of GitHub Desktop's `app/test/unit/round-test.ts`.
//!
//! GitHub Desktop's `round(value, decimals)` (`ui/lib/round.ts`) rounds to a
//! number of decimals (`Math.round((value + Number.EPSILON) * 10 ** n) /
//! 10 ** n`, plain `Math.round` for `decimals <= 0`); `formatBytes`,
//! `formatCompactNumber` and the cherry-pick progress use it. Corvene has no
//! such function: `format_bytes` (corvene-ui `format.rs`) prints a fixed
//! number of decimals and the progress values go through
//! `corvene_git::rebase_ops::format_rebase_value` (GitHub Desktop's
//! `formatRebaseValue`, clamped to 0..=1 with two decimals). The cases call
//! a stand-in and are ignored until it exists.

/// Stand-in for GitHub Desktop's `round(value, decimals)`. Replace it with
/// the Corvene function once there is one and remove the `#[ignore]`s.
fn round(_value: f64, _decimals: i32) -> f64 {
    unimplemented!("Corvene has no round(value, decimals)")
}

// GHD: unit/round-test.ts › round › rounds to the desired number decimals
#[test]
#[ignore = "ghd: missing: Corvene has no round(value, decimals) (ui/lib/round.ts)"]
fn rounds_to_the_desired_number_decimals() {
    assert_eq!(round(1.23456789, 0), 1.0);
    assert_eq!(round(1.23456789, 1), 1.2);
    assert_eq!(round(1.23456789, 2), 1.23);
    assert_eq!(round(1.23456789, 3), 1.235);
    assert_eq!(round(1.23456789, 4), 1.2346);
    assert_eq!(round(1.23456789, 5), 1.23457);
    assert_eq!(round(1.23456789, 6), 1.234568);
}

// GHD: unit/round-test.ts › round › doesn't attempt to round NaN
#[test]
#[ignore = "ghd: missing: Corvene has no round(value, decimals) (ui/lib/round.ts)"]
fn doesnt_attempt_to_round_nan() {
    assert!(round(f64::NAN, 1).is_nan());
}

// GHD: unit/round-test.ts › round › doesn't attempt to round infinity
#[test]
#[ignore = "ghd: missing: Corvene has no round(value, decimals) (ui/lib/round.ts)"]
fn doesnt_attempt_to_round_infinity() {
    assert!(!round(f64::INFINITY, 1).is_finite());
    assert!(!round(f64::NEG_INFINITY, 1).is_finite());
}

// GHD: unit/round-test.ts › round › doesn't attempt to round to less than zero decimals
#[test]
#[ignore = "ghd: missing: Corvene has no round(value, decimals) (ui/lib/round.ts)"]
fn doesnt_attempt_to_round_to_less_than_zero_decimals() {
    assert_eq!(round(1.23456789, 0), 1.0);
    assert_eq!(round(1.23456789, -1), 1.0);
    assert_eq!(round(1.23456789, -2), 1.0);
}
