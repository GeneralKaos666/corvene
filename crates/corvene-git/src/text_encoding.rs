//! Corvene `789-non-utf8-diffs`: diff and file text that is not UTF-8 (a
//! file kept in a legacy encoding such as windows-1252, ISO-8859-2 or
//! Shift_JIS) is decoded for display instead of showing U+FFFD replacement
//! characters. GHD decodes every diff and file as UTF-8 (`lib/diff-parser.ts`
//! over Node's `Buffer.toString()`), so such lines are unreadable there.
//!
//! A file with a `working-tree-encoding` attribute is UTF-8 in git's output
//! already (git converts it); only files git keeps in another encoding reach
//! this. Their encoding is guessed with chardetng (Firefox's detector),
//! which falls back to windows-1252. Diff lines keep their bytes
//! (`DiffLine::raw`) either way, so partial patches can write them back.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use encoding_rs::Encoding;

/// Set by the dispatcher from the flag.
static DECODE_LEGACY: AtomicBool = AtomicBool::new(false);

/// Turn the decoding on or off (flag `789-non-utf8-diffs`).
pub fn set_decode_legacy_text(enabled: bool) {
    DECODE_LEGACY.store(enabled, Ordering::Relaxed);
}

pub(crate) fn decode_legacy() -> bool {
    DECODE_LEGACY.load(Ordering::Relaxed)
}

/// The encoding non-UTF-8 `samples` (lines or whole files of one file) are
/// most likely in.
pub fn guess_encoding<'a>(samples: impl IntoIterator<Item = &'a [u8]>) -> &'static Encoding {
    // ISO-2022-JP is 7-bit, which is valid UTF-8 and never decoded here
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    for sample in samples {
        detector.feed(sample, false);
        detector.feed(b"\n", false);
    }
    detector.feed(b"", true);
    detector.guess(None, chardetng::Utf8Detection::Deny)
}

/// `bytes` in `encoding`, without BOM sniffing (a line has none).
pub fn decode_with(bytes: &[u8], encoding: &'static Encoding) -> String {
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

/// A whole file's bytes as text: UTF-16 text decoded while flag
/// `1306-utf16-diffs` is on ([`crate::utf16`]); UTF-8 as it is; otherwise
/// decoded in its guessed encoding while `789-non-utf8-diffs` is on, else
/// with replacement characters (GHD).
pub(crate) fn file_text(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(text) = crate::utf16::file_text(bytes) {
        return Cow::Owned(text);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) if decode_legacy() => Cow::Owned(decode_with(bytes, guess_encoding([bytes]))),
        Err(_) => String::from_utf8_lossy(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_and_decodes_legacy_text() {
        // "Größe – café" in windows-1252
        let latin = b"Gr\xf6\xdfe \x96 caf\xe9";
        let encoding = guess_encoding([&latin[..]]);
        assert_eq!(decode_with(latin, encoding), "Größe – café");
        // "日本語のテキスト" in Shift_JIS
        let sjis = b"\x93\xfa\x96{\x8c\xea\x82\xcc\x83e\x83L\x83X\x83g";
        let encoding = guess_encoding([&sjis[..]]);
        assert_eq!(encoding, encoding_rs::SHIFT_JIS);
        assert_eq!(decode_with(sjis, encoding), "日本語のテキスト");
    }
}
