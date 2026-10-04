//! Port of GitHub Desktop's `app/test/unit/git/ref-test.ts`.
//!
//! `formatAsLocalRef(name)` and `getSymbolicRef(repository, ref)`
//! (`lib/git/refs.ts`) are `corvene_git::format_as_local_ref` and
//! `corvene_git::get_symbolic_ref` (an `Err` is a rejection).

use corvene_git::format_as_local_ref;
use corvene_test_support::{git, setup_empty_repository};

/// `getSymbolicRef(repository, ref)`.
fn get_symbolic_ref(repository: &std::path::Path, reference: &str) -> Option<String> {
    corvene_git::get_symbolic_ref(git(), repository, reference).expect("getSymbolicRef")
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats the common branch syntax
#[test]
fn formats_the_common_branch_syntax() {
    let result = format_as_local_ref("master");
    assert_eq!(result, "refs/heads/master");
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats an explicit heads/ prefix
#[test]
fn formats_an_explicit_heads_prefix() {
    let result = format_as_local_ref("heads/something-important");
    assert_eq!(result, "refs/heads/something-important");
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats when a remote name is included
#[test]
fn formats_when_a_remote_name_is_included() {
    let result = format_as_local_ref("heads/Microsoft/master");
    assert_eq!(result, "refs/heads/Microsoft/master");
}

// GHD: unit/git/ref-test.ts › git/refs › getSymbolicRef › resolves a valid symbolic ref
#[test]
fn resolves_a_valid_symbolic_ref() {
    let repo = setup_empty_repository();
    let reference = get_symbolic_ref(repo.path(), "HEAD");
    assert_eq!(reference.as_deref(), Some("refs/heads/master"));
}

// GHD: unit/git/ref-test.ts › git/refs › getSymbolicRef › does not resolve a missing ref
#[test]
fn does_not_resolve_a_missing_ref() {
    let repo = setup_empty_repository();
    let reference = get_symbolic_ref(repo.path(), "FOO");
    assert!(reference.is_none());
}
