//! Helpers shared by `corvene-git`'s ports of GitHub Desktop's index tests
//! (`unit/git/checkout-test.ts`, `unit/git/gitignore-test.ts`).

/// `new RegExp(pattern).test(haystack)` for the patterns those GitHub
/// Desktop tests use, whose only special character is `.`: any
/// character but a line terminator (`\n`, `\r`, U+2028, U+2029). Every other
/// character of `pattern` matches itself (a newline in `pattern` matches a
/// newline).
pub fn regex_test(pattern: &str, haystack: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let haystack: Vec<char> = haystack.chars().collect();
    if pattern.len() > haystack.len() {
        return false;
    }
    (0..=haystack.len() - pattern.len()).any(|start| {
        pattern.iter().zip(&haystack[start..]).all(|(p, h)| {
            if *p == '.' {
                !matches!(h, '\n' | '\r' | '\u{2028}' | '\u{2029}')
            } else {
                p == h
            }
        })
    })
}

#[test]
fn regex_test_matches_like_a_dot_only_regex() {
    assert!(regex_test(
        "fatal: invalid reference: ..\n",
        "fatal: invalid reference: ..\n"
    ));
    assert!(!regex_test(
        "fatal: invalid reference: ..\n",
        "fatal: invalid reference: .."
    ));
    assert!(regex_test("a.c", "xxabcxx"));
    assert!(!regex_test("a.c", "a\nc"));
}
