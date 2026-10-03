//! Settings › Appearance › Formatting (GHD `models/formatting-preferences.ts`,
//! `lib/format-date.ts`, `lib/format-number.ts`): the date-fns patterns GHD
//! offers, rendered in local time, plus the number separators, GHD's
//! `formatNumber` / `formatCompactNumber` and `formatBytes`
//! (`ui/lib/bytes.ts`).

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use corvene_core::Settings;

#[derive(Clone, Debug)]
struct Prefs {
    date_format: String,
    time_format: String,
    thousands: String,
    decimal: String,
    prefer_absolute_dates: bool,
}

static PREFS: RwLock<Option<Prefs>> = RwLock::new(None);

/// Mirror the persisted settings (called whenever settings change).
pub fn sync(settings: &Settings) {
    let (thousands, decimal) = split_number_format(&settings.number_format);
    if let Ok(mut p) = PREFS.write() {
        *p = Some(Prefs {
            date_format: settings.date_format.clone(),
            time_format: settings.time_format.clone(),
            thousands,
            decimal,
            prefer_absolute_dates: settings.prefer_absolute_dates,
        });
    }
}

fn prefs() -> Prefs {
    PREFS
        .read()
        .ok()
        .and_then(|p| p.clone())
        .unwrap_or_else(|| Prefs {
            date_format: corvene_core::DEFAULT_DATE_FORMAT.into(),
            time_format: corvene_core::DEFAULT_TIME_FORMAT.into(),
            // Before the first `sync` (tests): GHD `defaultNumberFormat`
            // without a locale country, no grouping and a decimal point.
            thousands: String::new(),
            decimal: ".".into(),
            prefer_absolute_dates: false,
        })
}

/// GHD `getNumberFormatPreference`: the number format of Settings ›
/// Appearance › Formatting.
pub fn number_format() -> NumberFormat {
    let p = prefs();
    NumberFormat {
        thousands_separator: p.thousands,
        decimal_separator: p.decimal,
        maximum_fraction_digits: None,
    }
}

/// `preferAbsoluteDates`
pub fn prefer_absolute_dates() -> bool {
    prefs().prefer_absolute_dates
}

/// `numberFormatFromKey`: `"<thousands>|<decimal>"`.
pub fn split_number_format(key: &str) -> (String, String) {
    match key.split_once('|') {
        Some((t, d)) => (t.to_string(), d.to_string()),
        None => (",".into(), ".".into()),
    }
}

/// GHD `dateFormats` (models/formatting-preferences.ts).
pub const DATE_FORMATS: [&str; 16] = [
    "MMM d, yyyy",
    "MMMM do, yyyy",
    "MM/dd/yyyy",
    "dd/MM/yyyy",
    "dd-MM-yyyy",
    "dd.MM.yyyy",
    "yyyy/MM/dd",
    "yyyy-MM-dd",
    "yyyy.MM.dd",
    "MM/dd/yy",
    "dd/MM/yy",
    "dd-MM-yy",
    "dd.MM.yy",
    "yy/MM/dd",
    "yy-MM-dd",
    "yy.MM.dd",
];

/// GHD `timeFormats`.
pub const TIME_FORMATS: [&str; 8] = [
    "HH:mm:ss",
    "HH.mm.ss",
    "HH:mm",
    "HH.mm",
    "h:mm:ss aaa",
    "h.mm.ss aaa",
    "h:mm aaa",
    "h.mm aaa",
];

/// GHD `numberFormats` as `"<thousands>|<decimal>"` keys.
pub const NUMBER_FORMATS: [&str; 6] = ["|.", "|,", ",|.", ".|,", " |.", " |,"];

/// GHD `previewDate` (`new Date(2017, 9, 19, 14, 30, 45)`), which the
/// select examples are formatted with.
const PREVIEW: LocalTime = LocalTime {
    year: 2017,
    month: 10,
    day: 19,
    hour: 14,
    minute: 30,
    second: 45,
};

pub fn date_example(pattern: &str) -> String {
    format_pattern(pattern, &PREVIEW)
}

pub fn time_example(pattern: &str) -> String {
    format_pattern(pattern, &PREVIEW)
}

/// `formatNumber(1234567.89, format)` for the select.
pub fn number_example(key: &str) -> String {
    let (t, d) = split_number_format(key);
    format_number_with(1_234_567.89, &t, &d)
}

/// Broken-down local time.
#[derive(Clone, Copy, Debug)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// A path inside a repository as GitHub Desktop shows it: with the
/// platform's separator (it normalises paths, so `docs\old.md` on Windows).
pub fn display_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

/// Local wall-clock time via `localtime_r`.
pub fn local_time(at: SystemTime) -> LocalTime {
    let secs = at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // SAFETY: `localtime_r` fills the caller-provided `tm`; both pointers are valid.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let t: libc::time_t = secs as libc::time_t;
        #[cfg(not(windows))]
        libc::localtime_r(&t, &mut tm);
        // the CRT's equivalent, arguments swapped
        #[cfg(windows)]
        libc::localtime_s(&mut tm, &t);
        LocalTime {
            year: tm.tm_year + 1900,
            month: (tm.tm_mon + 1) as u32,
            day: tm.tm_mday as u32,
            hour: tm.tm_hour as u32,
            minute: tm.tm_min as u32,
            second: tm.tm_sec as u32,
        }
    }
}

/// Date in the user's date format.
pub fn format_date(at: SystemTime) -> String {
    format_pattern(&prefs().date_format, &local_time(at))
}

/// Date and time in the user's formats.
pub fn format_date_time(at: SystemTime) -> String {
    let p = prefs();
    let lt = local_time(at);
    format!(
        "{} {}",
        format_pattern(&p.date_format, &lt),
        format_pattern(&p.time_format, &lt)
    )
}

/// Integer with the user's thousands separator.
pub fn format_count(value: u64) -> String {
    format_number(value as f64, &number_format())
}

/// GHD `INumberFormat` (`models/formatting-preferences.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormat {
    pub thousands_separator: String,
    pub decimal_separator: String,
    /// `maximumFractionDigits`: decimal digits past this many are cut off
    /// (not rounded); with none left the decimal separator goes too.
    pub maximum_fraction_digits: Option<usize>,
}

impl NumberFormat {
    pub fn new(thousands: &str, decimal: &str) -> Self {
        Self {
            thousands_separator: thousands.to_string(),
            decimal_separator: decimal.to_string(),
            maximum_fraction_digits: None,
        }
    }
}

/// `String(value)` / `${value}` of a number JavaScript cannot write out as
/// digits: `NaN`, `Infinity`, `-Infinity`.
fn js_non_finite(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value > 0. {
        "Infinity".into()
    } else {
        "-Infinity".into()
    }
}

/// JavaScript's `Number.prototype.toString()` of a finite, non-negative
/// number: the shortest round-trip digits, in exponent form (`1e+21`,
/// `1.5e-7`) from 1e21 up and below 1e-6.
fn js_number_string(value: f64) -> String {
    if value != 0. && !(1e-6..1e21).contains(&value) {
        let exp = format!("{value:e}");
        if let Some((mantissa, e)) = exp.split_once('e') {
            return match e.strip_prefix('-') {
                Some(e) => format!("{mantissa}e-{e}"),
                None => format!("{mantissa}e+{e}"),
            };
        }
    }
    format!("{value}")
}

/// GHD `formatNumber(value, fmt)`: plain decimal expansion (JavaScript's
/// `toString`) with configurable separators, decimals cut to
/// `maximumFractionDigits`.
pub fn format_number(value: f64, fmt: &NumberFormat) -> String {
    if !value.is_finite() {
        return js_non_finite(value);
    }
    let negative = value < 0.;
    let text = js_number_string(value.abs());
    let (int_part, dec_part) = match text.split_once('.') {
        Some((i, d)) => (i.to_string(), Some(d.to_string())),
        None => (text, None),
    };
    let dec_part = match (dec_part, fmt.maximum_fraction_digits) {
        (Some(d), Some(max)) => {
            Some(d.chars().take(max).collect::<String>()).filter(|d| !d.is_empty())
        }
        (d, _) => d,
    };
    // `\B(?=(\d{3})+(?!\d))`: before every group of three digits that
    // ends a run of digits (so `1e+21` is left alone)
    let chars: Vec<char> = int_part.chars().collect();
    let mut grouped = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && chars[i - 1].is_ascii_digit() {
            let run = chars[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            if run > 0 && run.is_multiple_of(3) {
                grouped.push_str(&fmt.thousands_separator);
            }
        }
        grouped.push(*c);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(d) = dec_part {
        out.push_str(&fmt.decimal_separator);
        out.push_str(&d);
    }
    out
}

/// [`format_number`] with just the two separators.
pub fn format_number_with(value: f64, thousands: &str, decimal: &str) -> String {
    format_number(value, &NumberFormat::new(thousands, decimal))
}

/// GHD `defaultDecimalUnits`.
pub const DEFAULT_DECIMAL_UNITS: [&str; 5] = ["", "k", "m", "b", "t"];

/// GHD `ICompactFormatOptions` (`lib/format-number.ts`); `None` is GHD's
/// default for each.
#[derive(Clone, Debug, Default)]
pub struct CompactFormatOptions<'a> {
    /// Decimal places: one below 10 of a unit, none from 10 up by default.
    pub decimals: Option<u32>,
    /// 1000 (`k`, `m`, `b`, `t`, the default) or 1024 (`KiB`, `MiB`, …).
    pub base: Option<u32>,
    /// Unit suffixes from the base unit up, [`DEFAULT_DECIMAL_UNITS`] by
    /// default.
    pub units: Option<&'a [&'a str]>,
    /// Between the number and the unit, nothing by default.
    pub unit_separator: Option<&'a str>,
    /// The user's [`number_format`] by default.
    pub number_format: Option<NumberFormat>,
}

/// GHD `formatCompactNumber(value, fmt)`: `999`, `1.2k`, `12k`, `1.5m`,
/// `1,000t` (scaled to the largest unit at most, rounded half up with
/// GHD `round`, then [`format_number`]).
pub fn format_compact_number(value: f64, opts: &CompactFormatOptions) -> String {
    if !value.is_finite() {
        return js_non_finite(value);
    }
    let abs = value.abs();
    let base = f64::from(opts.base.unwrap_or(1000));
    let units = opts.units.unwrap_or(&DEFAULT_DECIMAL_UNITS);
    let separator = opts.unit_separator.unwrap_or("");
    let fmt = opts.number_format.clone().unwrap_or_else(number_format);
    if abs < base {
        let result = format_number(value, &fmt);
        // byte formatting shows the unit even for small values
        return match units.first() {
            Some(unit) if !unit.is_empty() => format!("{result}{separator}{unit}"),
            _ => result,
        };
    }
    let unit_ix = ((abs.ln() / base.ln()).floor() as usize).min(units.len().saturating_sub(1));
    let scaled = value / base.powi(unit_ix as i32);
    let decimals = match opts.decimals {
        Some(d) => i32::try_from(d).unwrap_or(i32::MAX),
        None if scaled.abs() < 10. => 1,
        None => 0,
    };
    let result = corvene_core::round::round(scaled, decimals);
    format!(
        "{}{separator}{}",
        format_number(result, &fmt),
        units.get(unit_ix).copied().unwrap_or("")
    )
}

/// The `.counter` text of GHD `FilesChangedBadge`
/// (`ui/changes/files-changed-badge.tsx`), the Changes tab's count:
/// `formatCompactNumber(filesChangedCount)`, so 3000 reads `3k`.
pub fn files_changed_badge(files_changed_count: usize) -> String {
    format_compact_number(files_changed_count as f64, &CompactFormatOptions::default())
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn ordinal(day: u32) -> String {
    let suffix = match (day % 10, day % 100) {
        (1, n) if n != 11 => "st",
        (2, n) if n != 12 => "nd",
        (3, n) if n != 13 => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// The subset of date-fns tokens GHD's patterns use.
pub fn format_pattern(pattern: &str, t: &LocalTime) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let hour12 = if t.hour.is_multiple_of(12) {
            12
        } else {
            t.hour % 12
        };
        match (c, run) {
            ('y', 4) => out.push_str(&format!("{:04}", t.year)),
            ('y', _) => out.push_str(&format!("{:02}", t.year % 100)),
            ('M', 4) => out.push_str(MONTHS[(t.month as usize - 1).min(11)]),
            ('M', 3) => out.push_str(&MONTHS[(t.month as usize - 1).min(11)][..3]),
            ('M', 2) => out.push_str(&format!("{:02}", t.month)),
            ('M', _) => out.push_str(&t.month.to_string()),
            ('d', 2) => out.push_str(&format!("{:02}", t.day)),
            ('d', _) => {
                // `do` = ordinal day
                if i + 1 < chars.len() && chars[i + 1] == 'o' {
                    out.push_str(&ordinal(t.day));
                    i += 2;
                    continue;
                }
                out.push_str(&t.day.to_string());
            }
            ('H', 2) => out.push_str(&format!("{:02}", t.hour)),
            ('H', _) => out.push_str(&t.hour.to_string()),
            ('h', 2) => out.push_str(&format!("{hour12:02}")),
            ('h', _) => out.push_str(&hour12.to_string()),
            ('m', _) => out.push_str(&format!("{:02}", t.minute)),
            ('s', _) => out.push_str(&format!("{:02}", t.second)),
            ('a', n) => {
                let am = t.hour < 12;
                out.push_str(match n {
                    3.. => {
                        if am {
                            "am"
                        } else {
                            "pm"
                        }
                    }
                    _ => {
                        if am {
                            "AM"
                        } else {
                            "PM"
                        }
                    }
                });
            }
            _ => {
                for _ in 0..run {
                    out.push(c);
                }
            }
        }
        i += run;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_ghd_examples() {
        assert_eq!(date_example("MMM d, yyyy"), "Oct 19, 2017");
        assert_eq!(date_example("MMMM do, yyyy"), "October 19th, 2017");
        assert_eq!(date_example("dd.MM.yy"), "19.10.17");
        assert_eq!(time_example("h:mm aaa"), "2:30 pm");
        assert_eq!(time_example("HH:mm:ss"), "14:30:45");
    }

    #[test]
    fn numbers_group_thousands() {
        assert_eq!(number_example(",|."), "1,234,567.89");
        assert_eq!(number_example(".|,"), "1.234.567,89");
        assert_eq!(number_example("|."), "1234567.89");
        assert_eq!(format_number_with(-42.0, ",", "."), "-42");
    }
}

/// GHD `formatBytes(bytes, decimals)` (`ui/lib/bytes.ts`):
/// [`format_compact_number`] in base 1024 with IEC units and a space, so
/// `1023 B`, `1.5 KiB` (trailing zeros dropped), `1.3 GiB` (halves round
/// up), in the user's number format.
pub fn format_bytes(bytes: i64, decimals: usize) -> String {
    const UNITS: [&str; 9] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];
    format_compact_number(
        bytes as f64,
        &CompactFormatOptions {
            decimals: Some(u32::try_from(decimals).unwrap_or(u32::MAX)),
            base: Some(1024),
            units: Some(&UNITS),
            unit_separator: Some(" "),
            number_format: None,
        },
    )
}

#[cfg(test)]
mod byte_tests {
    use super::*;

    #[test]
    fn formats_like_ghd() {
        assert_eq!(format_bytes(0, 0), "0 B");
        assert_eq!(format_bytes(1023, 0), "1023 B");
        assert_eq!(format_bytes(1536, 2), "1.5 KiB");
        assert_eq!(format_bytes(-2048, 1), "-2 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024, 2), "5 MiB");
        assert_eq!(format_bytes(1 << 50, 0), "1 PiB");
    }

    #[test]
    fn compact_numbers_and_badges() {
        assert_eq!(files_changed_badge(301), "301");
        assert_eq!(files_changed_badge(3000), "3k");
        assert_eq!(format_number(1e21, &NumberFormat::new(",", ".")), "1e+21");
        assert_eq!(
            format_number(1.5e-7, &NumberFormat::new(",", ".")),
            "1.5e-7"
        );
        assert_eq!(
            format_number(f64::INFINITY, &NumberFormat::new(",", ".")),
            "Infinity"
        );
    }
}
