//! Helpers shared by the `index` lane's `corvene-git` modules.

use std::path::Path;

use corvene_git::GitError;
use corvene_models::Branch;

/// GitHub Desktop's `GitError.message` (`lib/git/core.ts`) for a failed git
/// command: dugite's description of a recognised error, else git's output.
/// Corvene's text for it is `GitFailure::description` of the returned
/// error, else `GitFailure::output`; any other error is its `Display`.
pub fn git_error_message(err: &GitError) -> String {
    match err.failure() {
        Some(failure) => failure
            .description("Settings")
            .unwrap_or_else(|| failure.output.clone()),
        None => err.to_string(),
    }
}

/// `new RegExp(pattern).test(haystack)` for the patterns GitHub Desktop's
/// tests in this lane use, whose only special character is `.`: any
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

/// GitHub Desktop's `getBranches(repository, ...prefixes)`
/// (`lib/git/for-each-ref.ts`): `git for-each-ref` over `refs/heads` and
/// `refs/remotes` (or the given prefixes), symbolic refs left out.
///
/// Corvene reads the same branches with `corvene_git::open_repository`
/// (`RepositoryInfo::branches`, which leaves out `<remote>/HEAD`), as
/// `corvene-test-support`'s `get_branch_or_error` does. A prefix matches as
/// a `for-each-ref` pattern does: the whole ref name, or its leading
/// components.
pub fn get_branches(repository: &Path, prefixes: &[&str]) -> Vec<Branch> {
    let branches = corvene_git::open_repository(repository)
        .unwrap_or_else(|err| panic!("getBranches in {}: {err}", repository.display()))
        .branches;
    branches
        .into_iter()
        .filter(|b| {
            prefixes.is_empty()
                || prefixes.iter().any(|p| {
                    let p = p.trim_end_matches('/');
                    b.full_name == p
                        || b.full_name
                            .strip_prefix(p)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
        })
        .collect()
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
