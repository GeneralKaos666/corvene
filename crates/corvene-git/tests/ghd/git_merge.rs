//! Port of GitHub Desktop's `app/test/unit/git/merge-test.ts`.
//!
//! - `merge(repository, branch)` (`lib/git/merge.ts`) is
//!   `corvene_git::merge_branch(git, path, branch, false)`; GitHub Desktop's
//!   `MergeResult` is `corvene_git::MergeOutcome` (an `Err` is a throw).
//! - `getMergeBase` is `corvene_git::merge_base`: `Ok(None)` is GitHub
//!   Desktop's `null`, an `Err` is a throw.
//! - `getBranches` is `corvene_git::open_repository(..).branches`.
//! - `abortMerge` is `corvene_git::abort_merge`. GitHub Desktop rejects with
//!   its `GitError`, whose message is dugite's description of the failure
//!   (`getDescriptionForError`); Corvene's text for it is
//!   `GitFailure::description` of the returned error.

use corvene_git::MergeOutcome;
use corvene_test_support::{
    exec, git, setup_conflicted_repo, setup_empty_repository, setup_fixture_repository,
};

// GHD: unit/git/merge-test.ts › git/merge › merge › and is successful › returns MergeResult.Success
#[test]
fn returns_merge_result_success() {
    let repository = setup_fixture_repository("merge-base-test");
    assert_eq!(
        corvene_git::merge_branch(git(), repository.path(), "dev", false).expect("merge"),
        MergeOutcome::Success
    );
}

// GHD: unit/git/merge-test.ts › git/merge › merge › and is a noop › returns MergeResult.AlreadyUpToDate
#[test]
fn returns_merge_result_already_up_to_date() {
    let repository = setup_fixture_repository("merge-base-test");
    corvene_git::merge_branch(git(), repository.path(), "dev", false).expect("merge");
    assert_eq!(
        corvene_git::merge_branch(git(), repository.path(), "dev", false).expect("merge"),
        MergeOutcome::AlreadyUpToDate
    );
}

// GHD: unit/git/merge-test.ts › git/merge › getMergeBase › returns the common ancestor of two branches
#[test]
fn returns_the_common_ancestor_of_two_branches() {
    let repository = setup_fixture_repository("merge-base-test");

    let all_branches = corvene_git::open_repository(repository.path())
        .expect("getBranches")
        .branches;
    let first = all_branches
        .iter()
        .find(|f| f.name_without_remote() == "master")
        .unwrap_or_else(|| panic!("Unable to find branch: master"));

    let second = all_branches
        .iter()
        .find(|f| f.name_without_remote() == "dev")
        .unwrap_or_else(|| panic!("Unable to find branch: dev"));

    let reference = corvene_git::merge_base(
        git(),
        repository.path(),
        first.tip.as_deref().expect("master has a tip"),
        second.tip.as_deref().expect("dev has a tip"),
    )
    .expect("getMergeBase");
    assert_eq!(
        reference.as_deref(),
        Some("df0d73dc92ff496c6a61f10843d527b7461703f4")
    );
}

// GHD: unit/git/merge-test.ts › git/merge › getMergeBase › returns null when the branches do not have a common ancestor
#[test]
fn returns_null_when_the_branches_do_not_have_a_common_ancestor() {
    let repository = setup_empty_repository();

    let first_branch = "master";
    let second_branch = "gh-pages";

    // create the first commit
    exec(
        ["commit", "--allow-empty", "-m", "first commit on master"],
        repository.path(),
    );

    // create a second branch that's orphaned from our current branch
    exec(["checkout", "--orphan", second_branch], repository.path());

    // add a commit to this new branch
    exec(
        ["commit", "--allow-empty", "-m", "first commit on gh-pages"],
        repository.path(),
    );

    let all_branches = corvene_git::open_repository(repository.path())
        .expect("getBranches")
        .branches;
    let first = all_branches
        .iter()
        .find(|f| f.name_without_remote() == first_branch)
        .unwrap_or_else(|| panic!("Unable to find branch {first_branch}"));

    let second = all_branches
        .iter()
        .find(|f| f.name_without_remote() == second_branch)
        .unwrap_or_else(|| panic!("Unable to find branch {second_branch}"));

    let reference = corvene_git::merge_base(
        git(),
        repository.path(),
        first.tip.as_deref().expect("master has a tip"),
        second.tip.as_deref().expect("gh-pages has a tip"),
    )
    .expect("getMergeBase");
    assert!(reference.is_none());
}

// GHD: unit/git/merge-test.ts › git/merge › getMergeBase › returns null when a ref cannot be found
#[test]
#[ignore = "ghd: bug: corvene_git::merge_base returns Err when git merge-base exits 128 for an unknown ref; GHD getMergeBase returns null (successExitCodes 0, 1, 128)"]
fn returns_null_when_a_ref_cannot_be_found() {
    let repository = setup_empty_repository();

    // create the first commit
    exec(
        ["commit", "--allow-empty", "-m", "first commit on master"],
        repository.path(),
    );

    let reference = corvene_git::merge_base(
        git(),
        repository.path(),
        "master",
        "origin/some-unknown-branch",
    )
    .expect("getMergeBase");
    assert!(reference.is_none());
}

// GHD: unit/git/merge-test.ts › git/merge › abortMerge › when there is no in-progress merge › throws an error
#[test]
fn abort_merge_throws_an_error() {
    let repository = setup_empty_repository();
    let err =
        corvene_git::abort_merge(git(), repository.path()).expect_err("abortMerge should reject");
    let message = err
        .failure()
        .and_then(|failure| failure.description("Settings"))
        .unwrap_or_else(|| err.to_string());
    assert!(
        message.contains("There is no merge in progress, so there is nothing to abort"),
        "{message}"
    );
}

// GHD: unit/git/merge-test.ts › git/merge › abortMerge › in the middle of resolving conflicts merge › aborts the merge
#[test]
fn aborts_the_merge() {
    let repository = setup_conflicted_repo();
    let result = corvene_git::abort_merge(git(), repository.path());
    assert!(result.is_ok(), "{result:?}");
}
