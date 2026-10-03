//! Port of GitHub Desktop's `app/test/unit/git/fetch-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `getBranchesDifferingFromUpstream(repository)` followed by
//!   `fastForwardBranches(repository, eligibleBranches)`
//!   (`lib/git/{for-each-ref,fetch}.ts`): `corvene_git::fast_forward_branches`,
//!   which picks the branches itself (local branches other than the current
//!   one that are strictly behind their upstream; GitHub Desktop passes every
//!   differing one and lets `fetch` refuse the non-fast-forwards) and runs
//!   the same `fetch . --show-forced-updates --no-write-fetch-head --stdin`.
//!   The two GitHub Desktop calls are that one Corvene call.
//! - `getBranches(repository)`: the branches of
//!   `corvene_git::open_repository` ([`get_branches`]), as
//!   `corvene-test-support`'s `get_branch_or_error` reads them (local and
//!   remote branches, like GitHub Desktop's).
//! - GitHub Desktop's `Branch.upstream` is the short name (`origin/main`)
//!   and `branchWithName(branches, branch.upstream)` finds the remote branch
//!   of that name. Corvene's `Branch::upstream` is the full ref name
//!   (`refs/remotes/origin/main`) and `Branch::upstream_short()` is GitHub
//!   Desktop's short one, so `branch.upstream !== null` checks
//!   `Branch::upstream` and the lookup is
//!   `branch_with_name(branches, branch.upstream_short())`, by the remote
//!   branch's `name` as in GitHub Desktop. `Branch.tip.sha` is
//!   `Branch::tip`.

use std::path::Path;

use corvene_git::fast_forward_branches;
use corvene_models::Branch;
use corvene_test_support::{git, setup_fixture_repository};

/// GitHub Desktop's `getBranches(repository)` (`lib/git/for-each-ref.ts`).
fn get_branches(repository: &Path) -> Vec<Branch> {
    corvene_git::open_repository(repository)
        .unwrap_or_else(|err| panic!("getBranches in {}: {err}", repository.display()))
        .branches
}

/// GitHub Desktop's `branchWithName(branches, name)`: the first branch named
/// `name`.
fn branch_with_name<'a>(branches: &'a [Branch], name: &str) -> &'a Branch {
    branches
        .iter()
        .find(|branch| branch.name == name)
        .unwrap_or_else(|| panic!("no branch named {name}"))
}

/// GitHub Desktop's `branch.upstream` (the short name, `origin/main`).
fn upstream(branch: &Branch) -> &str {
    branch
        .upstream_short()
        .unwrap_or_else(|| panic!("branch {} has no upstream", branch.name))
}

/// `branch.tip.sha` (GitHub Desktop's branches always have one).
fn tip_sha(branch: &Branch) -> &str {
    branch
        .tip
        .as_deref()
        .unwrap_or_else(|| panic!("branch {} has no tip", branch.name))
}

// GHD: unit/git/fetch-test.ts › git/fetch › fastForwardBranches › fast-forwards branches using fetch
#[test]
fn fast_forwards_branches_using_fetch() {
    let repository = setup_fixture_repository("repo-with-non-updated-branches");

    fast_forward_branches(git(), repository.path())
        .unwrap_or_else(|err| panic!("fastForwardBranches: {err}"));

    let result_branches = get_branches(repository.path());

    // Only the branch behind was updated to match its upstream
    let branch_behind = branch_with_name(&result_branches, "branch-behind");
    assert!(branch_behind.upstream.is_some());

    let branch_behind_upstream = branch_with_name(&result_branches, upstream(branch_behind));
    assert_eq!(tip_sha(branch_behind_upstream), tip_sha(branch_behind));

    // The branch ahead is still ahead
    let branch_ahead = branch_with_name(&result_branches, "branch-ahead");
    assert!(branch_ahead.upstream.is_some());

    let branch_ahead_upstream = branch_with_name(&result_branches, upstream(branch_ahead));

    assert_ne!(tip_sha(branch_ahead_upstream), tip_sha(branch_ahead));

    // The branch ahead and behind is still ahead and behind
    let branch_ahead_and_behind = branch_with_name(&result_branches, "branch-ahead-and-behind");
    assert!(branch_ahead_and_behind.upstream.is_some());

    let branch_ahead_and_behind_upstream =
        branch_with_name(&result_branches, upstream(branch_ahead_and_behind));
    assert_ne!(
        tip_sha(branch_ahead_and_behind_upstream),
        tip_sha(branch_ahead_and_behind)
    );

    // The main branch hasn't been updated, since it's the current branch
    let main_branch = branch_with_name(&result_branches, "main");
    assert!(main_branch.upstream.is_some());

    let main_upstream = branch_with_name(&result_branches, upstream(main_branch));
    assert_ne!(tip_sha(main_upstream), tip_sha(main_branch));

    // The up-to-date branch is still matching its upstream
    let up_to_date_branch = branch_with_name(&result_branches, "branch-up-to-date");
    assert!(up_to_date_branch.upstream.is_some());
    let up_to_date_branch_upstream =
        branch_with_name(&result_branches, upstream(up_to_date_branch));
    assert_eq!(
        tip_sha(up_to_date_branch_upstream),
        tip_sha(up_to_date_branch)
    );
}

// We want to avoid messing with the FETCH_HEAD file. Normally, it shouldn't
// be something users would rely on, but we want to be good gitizens
// (:badpundog:) when possible.
// GHD: unit/git/fetch-test.ts › git/fetch › fastForwardBranches › does not change FETCH_HEAD after fast-forwarding branches with fetch
#[test]
fn does_not_change_fetch_head_after_fast_forwarding_branches_with_fetch() {
    let repository = setup_fixture_repository("repo-with-non-updated-branches");

    let fetch_head_path = repository.join(".git").join("FETCH_HEAD");
    let previous_fetch_head = std::fs::read_to_string(&fetch_head_path).unwrap();

    fast_forward_branches(git(), repository.path())
        .unwrap_or_else(|err| panic!("fastForwardBranches: {err}"));

    let current_fetch_head = std::fs::read_to_string(&fetch_head_path).unwrap();

    assert_eq!(current_fetch_head, previous_fetch_head);
}
