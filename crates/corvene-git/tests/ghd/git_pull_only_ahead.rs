//! Port of GitHub Desktop's `app/test/unit/git/pull/only-ahead-test.ts`.
//!
//! Corvene equivalents as in [`crate::git_pull_ahead_and_behind`]: `fetch`
//! / `pull` are `corvene_git::fetch` / `corvene_git::pull`
//! ([`crate::sync_support`]), `getAheadBehind(repository,
//! revSymmetricDifference(from, to))` is
//! `corvene_git::symmetric_ahead_behind`.

use corvene_git::symmetric_ahead_behind;
use corvene_models::{AheadBehind, Commit};
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, clone_repository, get_ref_or_error, get_tip_or_error, git,
    make_commit, repository_builder_pull::create_repository, setup_local_config,
};

use crate::sync_support::{FEATURE_BRANCH, fetch, pull, remote, remote_branch};

/// The `setup(t, pullFFConfig?)` of `describe('only ahead of tracking
/// branch')`: the clone gets one commit (so it is ahead), fetches, sets
/// `pull.ff` when given and pulls; returns the repository and the tips
/// before and after the pull.
fn setup(pull_ff_config: Option<&str>) -> (TestRepo, Commit, Commit) {
    let remote_repository = create_repository(FEATURE_BRANCH);
    let repository = clone_repository(&remote_repository);

    // add a commit to the local branch so that it is now "ahead"

    let changes_for_local_repository = Tree::with_message(
        "Added a new file to the local repository",
        [TreeEntry::new(
            "CONTRIBUTING.md",
            "# HELLO WORLD! \nTHINGS GO HERE\nYES, THINGS",
        )],
    );

    make_commit(&repository, &changes_for_local_repository);
    fetch(&repository, &remote());

    if let Some(pull_ff_config) = pull_ff_config {
        setup_local_config(&repository, [("pull.ff", pull_ff_config)]);
    }

    let previous_tip = get_tip_or_error(&repository);

    pull(&repository, &remote()).unwrap_or_else(|err| panic!("pull: {err}"));

    let new_tip = get_tip_or_error(&repository);

    (repository, previous_tip, new_tip)
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › by default › does not create new commit
#[test]
fn by_default_does_not_create_new_commit() {
    let (_repository, previous_tip, new_tip) = setup(None);
    assert_eq!(new_tip.sha, previous_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › by default › is different from tracking branch
#[test]
fn by_default_is_different_from_tracking_branch() {
    let (repository, _previous_tip, new_tip) = setup(None);

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_ne!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › by default › remains ahead of tracking branch
#[test]
fn by_default_remains_ahead_of_tracking_branch() {
    let (repository, _, _) = setup(None);

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();

    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 1,
            behind: 0
        })
    );
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=false set in config › does not create new commit
#[test]
fn ff_false_does_not_create_new_commit() {
    let (_repository, previous_tip, new_tip) = setup(Some("false"));
    assert_eq!(new_tip.sha, previous_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=false set in config › is different to tracking branch
#[test]
fn ff_false_is_different_to_tracking_branch() {
    let (repository, _previous_tip, new_tip) = setup(Some("false"));

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_ne!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=false set in config › is ahead of tracking branch
#[test]
fn ff_false_is_ahead_of_tracking_branch() {
    let (repository, _, _) = setup(Some("false"));

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();
    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 1,
            behind: 0
        })
    );
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=only set in config › does not create new commit
#[test]
fn ff_only_does_not_create_new_commit() {
    let (_repository, previous_tip, new_tip) = setup(Some("only"));
    assert_eq!(new_tip.sha, previous_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=only set in config › is different from tracking branch
#[test]
fn ff_only_is_different_from_tracking_branch() {
    let (repository, _previous_tip, new_tip) = setup(Some("only"));

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_ne!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/only-ahead-test.ts › git/pull › only ahead of tracking branch › with pull.ff=only set in config › is ahead of tracking branch
#[test]
fn ff_only_is_ahead_of_tracking_branch() {
    let (repository, _, _) = setup(Some("only"));

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();

    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 1,
            behind: 0
        })
    );
}
