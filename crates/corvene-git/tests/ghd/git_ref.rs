//! Port of GitHub Desktop's `app/test/unit/git/ref-test.ts`.
//!
//! Corvene has no equivalent of either function under test
//! (`lib/git/refs.ts`): `formatAsLocalRef` is written inline where Corvene
//! needs a local ref (`format!("refs/heads/{name}")`, without GitHub
//! Desktop's `heads/` handling), and `getSymbolicRef` exists only as the
//! remote-HEAD special case `corvene_git::remote_head`. The cases call
//! stand-ins and are ignored until the functions exist.

use std::path::Path;

use corvene_test_support::setup_empty_repository;

/// Stand-in for GitHub Desktop's `formatAsLocalRef(name)`: `name` as a
/// fully qualified local branch ref (`heads/x` → `refs/heads/x`, `x` →
/// `refs/heads/x`, `refs/heads/x` unchanged). Replace it with the Corvene
/// function once there is one and remove the `#[ignore]`s.
fn format_as_local_ref(_name: &str) -> String {
    unimplemented!("corvene_git has no formatAsLocalRef")
}

/// Stand-in for GitHub Desktop's `getSymbolicRef(repository, ref)`: the
/// canonical ref `ref` points at (`git symbolic-ref -q <ref>`), `None` when
/// it is missing or not symbolic. Replace it with the `corvene_git`
/// function once there is one and remove the `#[ignore]`s.
fn get_symbolic_ref(_repository: &Path, _reference: &str) -> Option<String> {
    unimplemented!("corvene_git has no getSymbolicRef")
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats the common branch syntax
#[test]
#[ignore = "ghd: missing: no formatAsLocalRef in Corvene (lib/git/refs.ts)"]
fn formats_the_common_branch_syntax() {
    let result = format_as_local_ref("master");
    assert_eq!(result, "refs/heads/master");
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats an explicit heads/ prefix
#[test]
#[ignore = "ghd: missing: no formatAsLocalRef in Corvene (lib/git/refs.ts)"]
fn formats_an_explicit_heads_prefix() {
    let result = format_as_local_ref("heads/something-important");
    assert_eq!(result, "refs/heads/something-important");
}

// GHD: unit/git/ref-test.ts › git/refs › formatAsLocalRef › formats when a remote name is included
#[test]
#[ignore = "ghd: missing: no formatAsLocalRef in Corvene (lib/git/refs.ts)"]
fn formats_when_a_remote_name_is_included() {
    let result = format_as_local_ref("heads/Microsoft/master");
    assert_eq!(result, "refs/heads/Microsoft/master");
}

// GHD: unit/git/ref-test.ts › git/refs › getSymbolicRef › resolves a valid symbolic ref
#[test]
#[ignore = "ghd: missing: corvene_git has no getSymbolicRef (lib/git/refs.ts); remote_head only reads refs/remotes/<remote>/HEAD"]
fn resolves_a_valid_symbolic_ref() {
    let repo = setup_empty_repository();
    let reference = get_symbolic_ref(repo.path(), "HEAD");
    assert_eq!(reference.as_deref(), Some("refs/heads/master"));
}

// GHD: unit/git/ref-test.ts › git/refs › getSymbolicRef › does not resolve a missing ref
#[test]
#[ignore = "ghd: missing: corvene_git has no getSymbolicRef (lib/git/refs.ts); remote_head only reads refs/remotes/<remote>/HEAD"]
fn does_not_resolve_a_missing_ref() {
    let repo = setup_empty_repository();
    let reference = get_symbolic_ref(repo.path(), "FOO");
    assert!(reference.is_none());
}
