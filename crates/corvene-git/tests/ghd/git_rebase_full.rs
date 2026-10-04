//! Port of GitHub Desktop's `app/test/unit/git/rebase-full-test.ts`.
//!
//! - `rebase`, `abortRebase` (`lib/git/rebase.ts`) are
//!   `crate::rebase_support::{rebase, abort_rebase}` over
//!   `corvene_git::rebase` / `corvene_git::abort_rebase`.
//! - `getRebaseInternalState` is `corvene_git::rebase_internal_state`
//!   (`None` is GitHub Desktop's `null`).
//! - `getBranches` is `corvene_git::open_repository(..).branches`;
//!   `getCommits(repo, 'HEAD', 10)` is `corvene_git::get_commits(path,
//!   "HEAD", 0, 10)`.
//! - `status.currentBranch` is `WorkingDirectoryStatus::branch`.

use corvene_git::RebaseResult;
use corvene_models::Branch;
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, create_branch, get_status_or_throw, make_commit,
    setup_empty_repository, switch_to,
};

use crate::rebase_support::{abort_rebase, rebase};

/// The test file's `findBranch(repo, name)`: the branch of `getBranches`
/// whose `name` is `name`.
fn find_branch(repo: &TestRepo, name: &str) -> Branch {
    let branches = corvene_git::open_repository(repo.path())
        .expect("getBranches")
        .branches;
    branches
        .into_iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("Branch {name} not found"))
}

// GHD: unit/git/rebase-full-test.ts › git/rebase › getRebaseInternalState › returns null when no rebase is in progress
#[test]
fn returns_null_when_no_rebase_is_in_progress() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("file.txt", "initial")]),
    );

    let state = corvene_git::rebase_internal_state(repo.path());
    assert_eq!(state, None);
}

// GHD: unit/git/rebase-full-test.ts › git/rebase › rebase › rebases a branch onto another
#[test]
fn rebases_a_branch_onto_another() {
    let repo = setup_empty_repository();

    // Create initial commit on master
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("base.txt", "base")]),
    );

    // Create feature branch with a commit
    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message(
            "feature commit",
            [TreeEntry::new("feature.txt", "feature work")],
        ),
    );

    // Add a commit on master
    switch_to(&repo, "master");
    make_commit(
        &repo,
        &Tree::with_message(
            "master commit",
            [TreeEntry::new("master.txt", "master work")],
        ),
    );

    // Switch to feature and rebase onto master
    switch_to(&repo, "feature");
    let master_branch = find_branch(&repo, "master");
    let feature_branch = find_branch(&repo, "feature");

    let result = rebase(&repo, &master_branch, &feature_branch, None);
    assert_eq!(result, RebaseResult::CompletedWithoutError);

    // Verify the feature branch now has all 3 commits
    let commits = corvene_git::get_commits(repo.path(), "HEAD", 0, 10).expect("getCommits");
    assert_eq!(commits.len(), 3);
}

// GHD: unit/git/rebase-full-test.ts › git/rebase › rebase › detects conflicts during rebase
#[test]
fn detects_conflicts_during_rebase() {
    let repo = setup_empty_repository();

    // Create initial commit
    make_commit(
        &repo,
        &Tree::with_message(
            "initial commit",
            [TreeEntry::new("conflict.txt", "base content")],
        ),
    );

    // Create feature branch with conflicting change
    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message(
            "feature change",
            [TreeEntry::new("conflict.txt", "feature version")],
        ),
    );

    // Make conflicting change on master
    switch_to(&repo, "master");
    make_commit(
        &repo,
        &Tree::with_message(
            "master change",
            [TreeEntry::new("conflict.txt", "master version")],
        ),
    );

    // Try to rebase feature onto master
    switch_to(&repo, "feature");
    let master_branch = find_branch(&repo, "master");
    let feature_branch = find_branch(&repo, "feature");

    let result = rebase(&repo, &master_branch, &feature_branch, None);
    assert_eq!(result, RebaseResult::ConflictsEncountered);
}

// GHD: unit/git/rebase-full-test.ts › git/rebase › abortRebase › aborts an in-progress rebase
#[test]
fn aborts_an_in_progress_rebase() {
    let repo = setup_empty_repository();

    // Set up conflicting branches
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("conflict.txt", "base")]),
    );

    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message(
            "feature change",
            [TreeEntry::new("conflict.txt", "feature")],
        ),
    );

    switch_to(&repo, "master");
    make_commit(
        &repo,
        &Tree::with_message("master change", [TreeEntry::new("conflict.txt", "master")]),
    );

    // Start rebase that will conflict
    switch_to(&repo, "feature");
    let master_branch = find_branch(&repo, "master");
    let feature_branch = find_branch(&repo, "feature");
    rebase(&repo, &master_branch, &feature_branch, None);

    // Abort the rebase
    abort_rebase(&repo);

    // Verify no rebase is in progress
    let state = corvene_git::rebase_internal_state(repo.path());
    assert_eq!(state, None);

    // Verify we're back to the original feature commit
    let status = get_status_or_throw(&repo);
    assert_eq!(status.branch.as_deref(), Some("feature"));
}
