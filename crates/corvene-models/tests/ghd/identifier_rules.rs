//! Port of GitHub Desktop's `app/test/unit/identifier-rules-test.ts`.
//!
//! GitHub Desktop's `gitAuthorNameIsValid` (`ui/lib/identifier-rules.ts`) is
//! `corvene_models::git_author_name_is_valid`. `String.fromCharCode(i)` for
//! `i <= 33` is the one-char string of that ASCII code.

use corvene_models::git_author_name_is_valid;

/// `String.fromCharCode(code)` for an ASCII `code`.
fn from_char_code(code: u8) -> String {
    char::from(code).to_string()
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns any value that is a disallowed character
#[test]
fn returns_any_value_that_is_a_disallowed_character() {
    assert!(!git_author_name_is_valid("."));
    assert!(!git_author_name_is_valid(","));
    assert!(!git_author_name_is_valid(":"));
    assert!(!git_author_name_is_valid(";"));
    assert!(!git_author_name_is_valid("<"));
    assert!(!git_author_name_is_valid(">"));
    assert!(!git_author_name_is_valid("\""));
    assert!(!git_author_name_is_valid("\\"));
    assert!(!git_author_name_is_valid("'"));
    assert!(!git_author_name_is_valid(" "));
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns true for empty strings
#[test]
fn returns_true_for_empty_strings() {
    assert!(git_author_name_is_valid(""));
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns false when name consists only of ascii character codes 0-32 inclusive
#[test]
fn returns_false_when_name_consists_only_of_ascii_character_codes_0_32_inclusive() {
    for i in 0..=32u8 {
        let char = from_char_code(i);
        assert!(!git_author_name_is_valid(&char));
    }
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns false when name consists solely of disallowed characters
#[test]
fn returns_false_when_name_consists_solely_of_disallowed_characters() {
    assert!(!git_author_name_is_valid(".;:<>"));
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns true if the value consists of allowed characters
#[test]
fn returns_true_if_the_value_consists_of_allowed_characters() {
    assert!(git_author_name_is_valid("this is great"));
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns true if the value contains allowed characters with disallowed characters
#[test]
fn returns_true_if_the_value_contains_allowed_characters_with_disallowed_characters() {
    let allowed = format!(";hi. there;{}", from_char_code(31));
    assert!(git_author_name_is_valid(&allowed));
}

// GHD: unit/identifier-rules-test.ts › Identifier rules › gitAuthorNameIsValid › returns true if the value contains ASCII characters whose code point is greater than 32
#[test]
fn returns_true_if_the_value_contains_ascii_characters_whose_code_point_is_greater_than_32() {
    let allowed = from_char_code(33);
    assert!(git_author_name_is_valid(&allowed));
}
