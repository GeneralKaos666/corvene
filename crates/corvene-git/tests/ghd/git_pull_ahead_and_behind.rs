//! Port of GitHub Desktop's `app/test/unit/git/pull/ahead-and-behind-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `fetch(repository, remote)` / `pull(repository, remote)`
//!   (`lib/git/{fetch,pull}.ts`): `corvene_git::fetch` / `corvene_git::pull`
//!   through [`crate::sync_support::fetch`] / [`crate::sync_support::pull`].
//! - `getAheadBehind(repository, revSymmetricDifference(from, to))`
//!   (`lib/git/rev-list.ts`): `corvene_git::symmetric_ahead_behind(git,
//!   path, from, to)`, `Some(AheadBehind)` for GitHub Desktop's
//!   `IAheadBehind` and `None` for its `null`.
//! - `getTipOrError` / `getRefOrError` (`helpers/git.ts`): the
//!   `corvene-test-support` helpers of the same name; `parentSHAs` is
//!   `Commit::parents`.

use corvene_git::symmetric_ahead_behind;
use corvene_models::{AheadBehind, Commit};
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, clone_repository, get_ref_or_error, get_tip_or_error, git,
    make_commit, repository_builder_pull::create_repository, setup_local_config,
};

use crate::sync_support::{FEATURE_BRANCH, fetch, pull, remote, remote_branch};

/// The `setup` of `describe('ahead and behind of tracking branch')`: a clone
/// and its source each get one commit, so the histories diverge, and the
/// clone fetches. The source stays alive with the clone.
fn setup() -> TestRepo {
    let remote_repository = create_repository(FEATURE_BRANCH);
    let repository = clone_repository(&remote_repository);

    // make a commits to both remote and local so histories diverge

    let changes_for_remote_repository = Tree::with_message(
        "Changed a file in the remote repository",
        [TreeEntry::new(
            "README.md",
            "# HELLO WORLD! \n WORDS GO HERE! \nLOL",
        )],
    );

    make_commit(&remote_repository, &changes_for_remote_repository);

    let changes_for_local_repository = Tree::with_message(
        "Added a new file to the local repository",
        [TreeEntry::new(
            "CONTRIBUTING.md",
            "# HELLO WORLD! \nTHINGS GO HERE\nYES, THINGS",
        )],
    );

    make_commit(&repository, &changes_for_local_repository);
    fetch(&repository, &remote());

    repository
}

/// `setupConfig` of a `describe`: [`setup`], then the local config, then a
/// pull, returning the repository and the tips before and after the pull.
fn setup_config(config: &[(&str, &str)]) -> (TestRepo, Commit, Commit) {
    let repository = setup();
    setup_local_config(&repository, config.iter().copied());

    let previous_tip = get_tip_or_error(&repository);

    pull(&repository, &remote()).unwrap_or_else(|err| panic!("pull: {err}"));

    let new_tip = get_tip_or_error(&repository);
    (repository, previous_tip, new_tip)
}

const REBASE_FALSE_FF_FALSE: &[(&str, &str)] = &[("pull.rebase", "false"), ("pull.ff", "false")];
const REBASE_FALSE: &[(&str, &str)] = &[("pull.rebase", "false")];
const REBASE_TRUE: &[(&str, &str)] = &[("pull.rebase", "true")];

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false and pull.ff=false set in config › creates a merge commit
#[test]
fn rebase_false_ff_false_creates_a_merge_commit() {
    let (_repository, previous_tip, new_tip) = setup_config(REBASE_FALSE_FF_FALSE);

    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.parents.len(), 2);
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false and pull.ff=false set in config › is different from remote branch
#[test]
fn rebase_false_ff_false_is_different_from_remote_branch() {
    let (repository, _previous_tip, new_tip) = setup_config(REBASE_FALSE_FF_FALSE);

    let remote_commit = get_ref_or_error(&repository, &remote_branch());
    assert_ne!(remote_commit.sha, new_tip.sha);
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false and pull.ff=false set in config › is ahead of tracking branch
#[test]
fn rebase_false_ff_false_is_ahead_of_tracking_branch() {
    let (repository, _, _) = setup_config(REBASE_FALSE_FF_FALSE);

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();
    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 2,
            behind: 0
        })
    );
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false set in config › creates a merge commit
#[test]
fn rebase_false_creates_a_merge_commit() {
    let (_repository, previous_tip, new_tip) = setup_config(REBASE_FALSE);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.parents.len(), 2);
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false set in config › is ahead of tracking branch
#[test]
fn rebase_false_is_ahead_of_tracking_branch() {
    let (repository, _, _) = setup_config(REBASE_FALSE);

    let ahead_behind =
        symmetric_ahead_behind(git(), repository.path(), FEATURE_BRANCH, &remote_branch()).unwrap();
    assert_eq!(
        ahead_behind,
        Some(AheadBehind {
            ahead: 2,
            behind: 0
        })
    );
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=true set in config › does not create a merge commit
#[test]
fn rebase_true_does_not_create_a_merge_commit() {
    let (_repository, previous_tip, new_tip) = setup_config(REBASE_TRUE);

    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.parents.len(), 1);
}

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=true set in config › is ahead of tracking branch
#[test]
fn rebase_true_is_ahead_of_tracking_branch() {
    let (repository, _, _) = setup_config(REBASE_TRUE);
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

// GHD: unit/git/pull/ahead-and-behind-test.ts › git/pull › ahead and behind of tracking branch › with pull.rebase=false and pull.ff=only set in config › throws an error as the user blocks merge commits on pull
#[test]
fn throws_an_error_as_the_user_blocks_merge_commits_on_pull() {
    let repository = setup();
    setup_local_config(&repository, [("pull.rebase", "false"), ("pull.ff", "only")]);
    assert!(pull(&repository, &remote()).is_err());
}
