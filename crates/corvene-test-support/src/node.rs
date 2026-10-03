//! The Node.js and JavaScript built-ins GitHub Desktop's tests use, where
//! Rust's standard library has no one-call equivalent: `fs.writeFile` /
//! `fs.appendFile`, `Buffer.toString('base64')`, `Date.parse` and
//! `Date.prototype.toISOString`.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Node's `fs.writeFile(path, contents)`: create or truncate the file.
///
/// # Panics
///
/// When the file cannot be written (the promise rejects).
pub fn write_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::write(path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Node's `fs.appendFile(path, contents)`: append, creating the file when
/// it does not exist.
///
/// # Panics
///
/// When the file cannot be opened or written (the promise rejects).
pub fn append_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
    let path = path.as_ref();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    file.write_all(contents.as_ref())
        .unwrap_or_else(|e| panic!("append to {}: {e}", path.display()));
}

/// Node's `Buffer.from(bytes).toString('base64')`: standard alphabet, with
/// `=` padding.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// JavaScript's `Date.parse(iso)` / `new Date(iso)` for the UTC timestamps
/// GitHub Desktop's tests write: `YYYY-MM-DDTHH:MM:SS`, optional fraction
/// (milliseconds are kept, finer digits dropped, as JavaScript's dates do),
/// then `Z`. Times before 1970 are not supported.
///
/// # Panics
///
/// When `iso` is not of that form.
pub fn date_parse(iso: &str) -> SystemTime {
    fn bad(iso: &str) -> ! {
        panic!("date_parse: not a UTC ISO-8601 timestamp: {iso:?}")
    }
    let bytes = iso.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !iso.ends_with('Z')
    {
        bad(iso);
    }
    let number = |range: std::ops::Range<usize>| -> i64 {
        match iso.get(range) {
            Some(digits) if digits.bytes().all(|b| b.is_ascii_digit()) => {
                digits.parse().unwrap_or_else(|_| bad(iso))
            }
            _ => bad(iso),
        }
    };
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let (hour, minute, second) = (number(11..13), number(14..16), number(17..19));
    let millis: u64 = match iso[19..iso.len() - 1].strip_prefix('.') {
        None if iso.len() == 20 => 0,
        Some(digits) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{digits:0<3}")[..3]
                .parse()
                .unwrap_or_else(|_| bad(iso))
        }
        _ => bad(iso),
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        bad(iso);
    }
    // Howard Hinnant's days_from_civil
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    let seconds = u64::try_from(seconds).unwrap_or_else(|_| bad(iso));
    UNIX_EPOCH + Duration::from_secs(seconds) + Duration::from_millis(millis)
}

/// JavaScript's `date.toISOString()`: UTC, milliseconds, `Z`
/// (`2025-01-02T00:00:00.000Z`), for times after the epoch.
///
/// # Panics
///
/// When `date` is before the epoch.
pub fn to_iso_string(date: SystemTime) -> String {
    let since = date
        .duration_since(UNIX_EPOCH)
        .expect("to_iso_string: a time after the epoch");
    let millis = since.subsec_millis();
    let secs = i64::try_from(since.as_secs()).expect("to_iso_string: a representable time");
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
