//! Port of GitHub Desktop's `app/test/unit/git/format-patch-test.ts`.
//!
//! GitHub Desktop's `formatPatch(repository, base, head)`
//! (`lib/git/format-patch.ts`: `git format-patch --unified=1 --minimal
//! --stdout <base>..<head>`, the patch series as one string) has no Corvene
//! equivalent. `corvene_git::format_patches` (Create Patch File…, flag
//! `821-create-patch-files`) writes one file per listed commit with `git
//! format-patch -1 --no-signature -o <dir>` and takes no range, and
//! `corvene_git::format_patch` is GitHub Desktop's other `formatPatch`
//! (`lib/patch-formatter.ts`, a diff selection as a patch). The cases call
//! a stand-in and are ignored until Corvene has the range form; GitHub
//! Desktop 3.6.6 itself no longer calls it from `app/src`.
//!
//! - `typeof patch === 'string'` holds for any value of the stand-in's
//!   `String` result.
//! - "will be applied cleanly" asserts `assert(result)`, which only checks
//!   that dugite's `exec` resolved; `exec_with` returning is the same check.

use std::path::Path;

use corvene_git::error::Result;
use corvene_test_support::{
    ExecOptions, TestRepo, Tree, TreeEntry, clone_local_repository, exec, exec_with, make_commit,
    setup_fixture_repository, setup_two_commit_repo,
};

/// Stand-in for GitHub Desktop's `formatPatch(repository, base, head)`
/// (`lib/git/format-patch.ts`). Replace it with the `corvene_git` function
/// once there is one and remove the `#[ignore]`s.
fn format_patch(_repository: &Path, _base: &str, _head: &str) -> Result<String> {
    unimplemented!("corvene_git has no formatPatch(repository, base, head)")
}

/// The `setup` of `in a repo with commits`.
fn setup() -> TestRepo {
    let repository = setup_two_commit_repo();
    make_commit(
        &repository,
        &Tree::new([TreeEntry::new("another-one", "dusty")]),
    );
    repository
}

// GHD: unit/git/format-patch-test.ts › formatPatch › in a repo with commits › returns a string for a single commit range
#[test]
#[ignore = "ghd: missing: corvene_git has no formatPatch(repository, base, head) (lib/git/format-patch.ts)"]
fn returns_a_string_for_a_single_commit_range() {
    let repository = setup();
    let patch: String = format_patch(repository.path(), "HEAD~", "HEAD").unwrap();
    assert_ne!(patch.len(), 0, "Expected patch to be empty");
}

// GHD: unit/git/format-patch-test.ts › formatPatch › in a repo with commits › returns a string for a multi commit range
#[test]
#[ignore = "ghd: missing: corvene_git has no formatPatch(repository, base, head) (lib/git/format-patch.ts)"]
fn returns_a_string_for_a_multi_commit_range() {
    let repository = setup();
    let patch: String = format_patch(repository.path(), "HEAD~2", "HEAD").unwrap();
    assert_ne!(patch.len(), 0, "Expected patch to be empty");
}

// GHD: unit/git/format-patch-test.ts › formatPatch › in a repo with commits › returns empty string for no range
#[test]
#[ignore = "ghd: missing: corvene_git has no formatPatch(repository, base, head) (lib/git/format-patch.ts)"]
fn returns_empty_string_for_no_range() {
    let repository = setup();
    let patch: String = format_patch(repository.path(), "HEAD", "HEAD").unwrap();
    assert_eq!(patch.len(), 0, "Expected patch to be empty");
}

// GHD: unit/git/format-patch-test.ts › formatPatch › in a repo with commits › applied in a related repo › will be applied cleanly
#[test]
#[ignore = "ghd: missing: corvene_git has no formatPatch(repository, base, head) (lib/git/format-patch.ts)"]
fn will_be_applied_cleanly() {
    let repository = setup();
    let cloned_repository = clone_local_repository(&repository);
    make_commit(
        &cloned_repository,
        &Tree::new([TreeEntry::new("okay-file", "okay")]),
    );

    let patch = format_patch(repository.path(), "HEAD~", "HEAD").unwrap();
    let _result = exec_with(
        ["apply"],
        cloned_repository.path(),
        ExecOptions {
            stdin: Some(patch.into_bytes()),
            ..Default::default()
        },
    );
}

// GHD: unit/git/format-patch-test.ts › formatPatch › in a repo with 105 commits › can create a series of commits from start to HEAD
#[test]
#[ignore = "ghd: missing: corvene_git has no formatPatch(repository, base, head) (lib/git/format-patch.ts)"]
fn can_create_a_series_of_commits_from_start_to_head() {
    let repository = setup_fixture_repository("repository-with-105-commits");
    let stdout = exec(["rev-list", "--max-parents=0", "HEAD"], repository.path()).stdout;
    let first_commit = stdout.trim();

    let _patch: String = format_patch(repository.path(), first_commit, "HEAD").unwrap();
}
