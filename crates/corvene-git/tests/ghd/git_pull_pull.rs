//! Port of GitHub Desktop's `app/test/unit/git/pull/pull-test.ts`.
//!
//! GitHub Desktop's `pull(repository, remote)` (`lib/git/pull.ts`, which
//! passes `--recurse-submodules`) is `corvene_git::pull` with submodules
//! followed ([`crate::sync_support::pull`]; `250-sync-skips-submodules` off,
//! its GitHub Desktop value).

use std::ffi::OsStr;

use corvene_models::Remote;
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, clone_repository, exec, make_commit, setup_empty_repository,
};

use crate::sync_support::pull;

/// GitHub Desktop's `setupRepositoryWithSubmodule(t)`: a parent repository
/// with one commit and the submodule `test-submodule` (a repository with
/// two commits) added and committed.
fn setup_repository_with_submodule() -> (TestRepo, TestRepo) {
    let parent = setup_empty_repository();
    let submodule = setup_empty_repository();

    // Add commits to submodule
    make_commit(
        &submodule,
        &Tree::with_message(
            "Initial commit in submodule",
            [TreeEntry::new("submodule-file.txt", "hello from submodule")],
        ),
    );

    make_commit(
        &submodule,
        &Tree::with_message(
            "Second commit in submodule",
            [TreeEntry::new("submodule-file.txt", "updated content")],
        ),
    );

    // Add commits to parent
    make_commit(
        &parent,
        &Tree::with_message(
            "Initial commit in parent",
            [TreeEntry::new("README.md", "# Parent repo")],
        ),
    );

    // Add submodule to parent
    exec(
        [
            OsStr::new("-c"),
            OsStr::new("protocol.file.allow=always"),
            OsStr::new("submodule"),
            OsStr::new("add"),
            submodule.path().as_os_str(),
            OsStr::new("test-submodule"),
        ],
        parent.path(),
    );

    exec(["commit", "-m", "Add submodule"], parent.path());

    (parent, submodule)
}

/// `stdout.trim().split('\n').length` of `git log --oneline`.
fn commit_count(stdout: &str) -> usize {
    stdout.trim().split('\n').count()
}

// GHD: unit/git/pull/pull-test.ts › git/pull › with submodules › updates submodule references after pulling changes
#[test]
fn updates_submodule_references_after_pulling_changes() {
    // Setup: Create parent with submodule, clone it
    let (parent, submodule) = setup_repository_with_submodule();

    let cloned = clone_repository(&parent);

    // Initialize submodules in the cloned repo
    exec(
        [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        ],
        cloned.path(),
    );

    let submodule_path = cloned.join("test-submodule");

    // Verify initial state
    let initial_log = exec(["log", "--oneline"], &submodule_path);
    let initial_commit_count = commit_count(&initial_log.stdout);
    assert_eq!(initial_commit_count, 2, "Should start with 2 commits");

    // Add a new commit to the submodule
    make_commit(
        &submodule,
        &Tree::with_message(
            "Third commit in submodule",
            [TreeEntry::new("another-file.txt", "more content")],
        ),
    );

    // Update the submodule reference in parent and commit
    exec(
        [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--remote",
        ],
        parent.path(),
    );
    exec(["add", "test-submodule"], parent.path());
    exec(
        ["commit", "-m", "Update submodule reference"],
        parent.path(),
    );

    let remote = Remote {
        name: "origin".into(),
        url: parent.path().to_string_lossy().into_owned(),
    };

    // Pull the changes
    pull(&cloned, &remote).unwrap_or_else(|err| panic!("pull: {err}"));

    // Verify submodule was updated to the new reference
    let final_log = exec(["log", "--oneline"], &submodule_path);
    let final_commit_count = commit_count(&final_log.stdout);

    assert_eq!(
        final_commit_count, 3,
        "Submodule should now have 3 commits after update"
    );
}

// GHD: unit/git/pull/pull-test.ts › git/pull › with submodules › handles pull when there are no submodule changes
#[test]
fn handles_pull_when_there_are_no_submodule_changes() {
    let (parent, _submodule) = setup_repository_with_submodule();
    let cloned = clone_repository(&parent);

    // Initialize submodules
    exec(
        [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        ],
        cloned.path(),
    );

    // Make a change that doesn't affect submodules
    make_commit(
        &parent,
        &Tree::with_message(
            "Update README again",
            [TreeEntry::new("README.md", "# Another update")],
        ),
    );

    let remote = Remote {
        name: "origin".into(),
        url: parent.path().to_string_lossy().into_owned(),
    };

    let submodule_path = cloned.join("test-submodule");
    let before_log = exec(["log", "--oneline"], &submodule_path);
    let before_count = commit_count(&before_log.stdout);

    // Pull should succeed without errors
    pull(&cloned, &remote).unwrap_or_else(|err| panic!("pull: {err}"));

    // Submodule should remain unchanged
    let after_log = exec(["log", "--oneline"], &submodule_path);
    let after_count = commit_count(&after_log.stdout);

    assert_eq!(
        after_count, before_count,
        "Submodule commits should remain unchanged"
    );
}
