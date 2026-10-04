//! Port of GitHub Desktop's `app/test/unit/git/pull/only-behind-test.ts`.
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

/// The `setup(t, config?)` of `describe('only behind tracking branch')`: the
/// clone's source gets two commits (so the clone is behind), the clone
/// fetches, applies `config` when given and pulls; returns the repository
/// and the tips before and after the pull.
fn setup(config: Option<&[(&str, &str)]>) -> (TestRepo, Commit, Commit) {
    let remote_repository = create_repository(FEATURE_BRANCH);
    let repository = clone_repository(&remote_repository);

    // make commits to remote is ahead of local repository

    let first_commit = Tree::with_message(
        "Changed a file in the remote repository",
        [TreeEntry::new(
            "README.md",
            "# HELLO WORLD! \n WORDS GO HERE! \nLOL",
        )],
    );

    let second_commit = Tree::with_message(
        "Added a new file to the remote repository",
        [TreeEntry::new(
            "CONTRIBUTING.md",
            "# HELLO WORLD! \nTHINGS GO HERE\nYES, THINGS",
        )],
    );

    make_commit(&remote_repository, &first_commit);
    make_commit(&remote_repository, &second_commit);

    fetch(&repository, &remote());

    if let Some(config) = config {
        setup_local_config(&repository, config.iter().copied());
    }

    let previous_tip = get_tip_or_error(&repository);

    pull(&repository, &remote()).unwrap_or_else(|err| panic!("pull: {err}"));

    let new_tip = get_tip_or_error(&repository);

    (repository, previous_tip, new_tip)
}

const REBASE_FALSE_FF_FALSE: &[(&str, &str)] = &[("pull.rebase", "false"), ("pull.ff", "false")];
const FF_ONLY: &[(&str, &str)] = &[("pull.ff", "only")];

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.rebase=false and pull.ff=false set in config › creates a merge commit
#[test]
fn rebase_false_ff_false_creates_a_merge_commit() {
    let (_repository, previous_tip, new_tip) = setup(Some(REBASE_FALSE_FF_FALSE));

    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.parents.len(), 2);
}

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.rebase=false and pull.ff=false set in config › is different from remote branch
#[test]
fn rebase_false_ff_false_is_different_from_remote_branch() {
    let (repository, _previous_tip, new_tip) = setup(Some(REBASE_FALSE_FF_FALSE));

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_ne!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.rebase=false and pull.ff=false set in config › is now ahead of tracking branch
#[test]
fn rebase_false_ff_false_is_now_ahead_of_tracking_branch() {
    let (repository, _, _) = setup(Some(REBASE_FALSE_FF_FALSE));

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

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.ff=only set in config › does not create a merge commit
#[test]
fn ff_only_does_not_create_a_merge_commit() {
    let (_repository, previous_tip, new_tip) = setup(Some(FF_ONLY));

    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.parents.len(), 1);
}

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.ff=only set in config › is same as remote branch
#[test]
fn ff_only_is_same_as_remote_branch() {
    let (repository, _previous_tip, new_tip) = setup(Some(FF_ONLY));

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_eq!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/only-behind-test.ts › git/pull › only behind tracking branch › with pull.ff=only set in config › is not behind tracking branch
#[test]
fn ff_only_is_not_behind_tracking_branch() {
    let (repository, _, _) = setup(Some(FF_ONLY));

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();
    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
}
