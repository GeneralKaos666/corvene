//! Port of GitHub Desktop's `app/test/unit/format-test.ts`.
//!
//! GitHub Desktop's `formatRebaseValue(value)` (`lib/rebase.ts`, the rebase
//! progress fraction) is [`corvene_git::rebase_ops::format_rebase_value`],
//! which takes and returns an `f32` (Corvene's progress values are `f32`).

use corvene_git::rebase_ops::format_rebase_value;

// GHD: unit/format-test.ts › format › formatRebaseValue › clamps a negative value
#[test]
fn clamps_a_negative_value() {
    let value = -1.;

    let result = format_rebase_value(value);

    assert_eq!(result, 0.);
}

// GHD: unit/format-test.ts › format › formatRebaseValue › clamps a positive value
#[test]
fn clamps_a_positive_value() {
    let value = 3.;

    let result = format_rebase_value(value);

    assert_eq!(result, 1.);
}

// GHD: unit/format-test.ts › format › formatRebaseValue › formats to two significant figures
#[test]
fn formats_to_two_significant_figures() {
    let value = 1. / 9.;

    let result = format_rebase_value(value);

    assert_eq!(result, 0.11);
}

// GHD: unit/format-test.ts › format › formatRebaseValue › handles infinity
#[test]
fn handles_infinity() {
    let value = 1. / 0.;

    let result = format_rebase_value(value);

    assert!(result >= 0.);
    assert!(result <= 1.);
}
