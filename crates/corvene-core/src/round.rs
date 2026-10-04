//! Port of GHD `ui/lib/round.ts`: round a number to a number of decimals,
//! as `formatBytes` (`ui/lib/bytes.ts`), `formatCompactNumber`
//! (`lib/format-number.ts`) and the cherry-pick progress
//! (`lib/git/cherry-pick.ts`) do.

/// JavaScript's `Math.round`: the nearest integer, halves rounded up
/// (towards positive infinity, so `-2.5` gives `-2`, where `f64::round`
/// gives `-3`). NaN and the infinities pass through.
pub fn math_round(value: f64) -> f64 {
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// GHD `round(value, decimals)`: `value` rounded to `decimals` decimals
/// (`1234.56789`, 2 → `1234.57`). Zero or fewer decimals round to an
/// integer; NaN and the infinities pass through.
pub fn round(value: f64, decimals: i32) -> f64 {
    if decimals <= 0 {
        return math_round(value);
    }
    let factor = 10f64.powi(decimals);
    math_round((value + f64::EPSILON) * factor) / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_halves_up_like_javascript() {
        assert_eq!(math_round(2.5), 3.0);
        assert_eq!(math_round(-2.5), -2.0);
        assert_eq!(math_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(round(1.005, 2), 1.01);
        assert_eq!(round(1_234.567_89, 3), 1_234.568);
    }
}
