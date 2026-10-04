//! Port of GitHub Desktop's `app/test/unit/truncate-with-ellipsis-test.ts`.
//!
//! GitHub Desktop's `truncateWithEllipsis(str, maxLength)`
//! (`lib/truncate-with-ellipsis.ts`) is
//! `corvene_core::notifications::truncate_with_ellipsis(text, max)`, which
//! the pull request notifications use as GitHub Desktop's
//! `notifications-store.ts` does. (corvene-ui's merge / rebase branch
//! dialogs have a private copy of their own, `dialogs/branch_dialogs.rs`.)

use corvene_core::notifications::truncate_with_ellipsis;

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › does not truncate a string that fits
#[test]
fn does_not_truncate_a_string_that_fits() {
    let str = "short";
    let result = truncate_with_ellipsis(str, 25);
    assert_eq!(result, str);
}

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › does not truncate a max length string
#[test]
fn does_not_truncate_a_max_length_string() {
    let str = "this-is-max-length-string";
    // `str.length`: UTF-16 code units
    let result = truncate_with_ellipsis(str, str.encode_utf16().count());
    assert_eq!(result, str);
}

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › truncates a string that does not fit
#[test]
fn truncates_a_string_that_does_not_fit() {
    let str = "this-string-exceeds-max-length";
    let result = truncate_with_ellipsis(str, 25);
    assert_eq!(result, "this-string-exceeds-max-l…");
}

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › does not truncate a unicode string that fits
#[test]
fn does_not_truncate_a_unicode_string_that_fits() {
    let str = "🌝🌛🌜🌚🌕🌖🌗🌘🌑🌒🌓🌔☀\u{FE0F}";
    let result = truncate_with_ellipsis(str, 25);
    assert_eq!(result, str);
}

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › does not truncate a max length unicode string
#[test]
fn does_not_truncate_a_max_length_unicode_string() {
    let str = "🌝🌛🌜🌚🌕🌖🌗🌘🌑🌒🌓🌔☀\u{FE0F}🌤⛅\u{FE0F}🌥☁\u{FE0F}🌦🌧⛈🌩🌨";
    let result = truncate_with_ellipsis(str, 22);
    assert_eq!(result, str);
}

// GHD: unit/truncate-with-ellipsis-test.ts › truncateWithEllipsis › truncates a unicode string that does not fit
#[test]
fn truncates_a_unicode_string_that_does_not_fit() {
    let str = "🌝🌛🌜🌚🌕🌖🌗🌘🌑🌒🌓🌔☀\u{FE0F}🌤⛅\u{FE0F}🌥☁\u{FE0F}🌦🌧⛈🌩🌨";
    let result = truncate_with_ellipsis(str, 13);
    assert_eq!(result, "🌝🌛🌜🌚🌕🌖🌗🌘🌑🌒🌓🌔☀\u{FE0F}…");
}
