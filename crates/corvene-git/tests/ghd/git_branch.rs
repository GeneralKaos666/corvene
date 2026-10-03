//! Port of GitHub Desktop's `app/test/unit/git/branch-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `GitStore.loadStatus()` + `store.tip` (`lib/stores/git-store.ts`):
//!   `corvene_git::open_repository(path).tip`, which the dispatcher's
//!   refresh stores as the repository's tip. GitHub Desktop's `TipState`
//!   kinds are the `corvene_models::Tip` variants; `IUnbornRepository.ref`
//!   is `Tip::Unborn::name`, `IDetachedHead.currentSha` is
//!   `Tip::Detached::sha`.
//! - `Branch.upstream` (the short `remote/branch`): `Branch::upstream_short`
//!   (Corvene's `Branch::upstream` holds the full ref name);
//!   `Branch.upstreamRemoteName`: `Branch::upstream_remote_name`;
//!   `Branch.tip.sha`: `Branch::tip`. The fixtures' remotes have no `/` in
//!   their names, so flag `256-remote-names-with-slashes` changes nothing.
//! - `Branch.upstreamWithoutRemote` (`models/branch.ts`): no accessor in
//!   `corvene_models::Branch`; [`upstream_without_remote`] is a stand-in. The
//!   `deleteRemoteBranch` cases only pass it on, so they derive it as
//!   `Dispatcher::delete_branch` does before it calls
//!   `corvene_git::delete_remote_branch` (`upstream_short` split at the first
//!   `/`).
//! - `getBranchesPointedAt` (`lib/git/branch.ts`, used for the "theirs"
//!   branch of merge conflicts): none ([`get_branches_pointed_at`] is a
//!   stand-in). `corvene_git::rebase_ops::branch_at` answers a different
//!   question (one local or remote branch at a sha, for the rebase flow).
//! - `createBranch`, `deleteLocalBranch`, `deleteRemoteBranch`,
//!   `checkoutBranch`: `corvene_git::{create_branch, delete_local_branch,
//!   delete_remote_branch, checkout_branch}`; `getBranches`:
//!   [`get_branches`]; GitHub Desktop's `git([...])` helper: `exec_ok`.

use std::path::Path;

use corvene_git::{checkout_branch, create_branch, delete_local_branch, delete_remote_branch};
use corvene_models::{Branch, Tip};
use corvene_test_support::{
    exec, exec_ok, get_branches, git, load_tip, setup_empty_repository, setup_fixture_repository,
    setup_local_fork_of_repository,
};

/// Stand-in for GitHub Desktop's `Branch.upstreamWithoutRemote`
/// (`models/branch.ts`: `removeRemotePrefix(upstream)`). Replace it with
/// the `corvene_models::Branch` accessor once there is one and remove the
/// `#[ignore]`.
fn upstream_without_remote(_branch: &Branch) -> Option<String> {
    unimplemented!("corvene_models::Branch has no upstreamWithoutRemote")
}

/// Stand-in for GitHub Desktop's `getBranchesPointedAt(repository,
/// commitish)` (`lib/git/branch.ts`): the names of the local branches whose
/// tip is `commitish` (`git branch --points-at`), `None` when git fails
/// (exit code 1 or 129). Replace it with the `corvene_git` function once
/// there is one and remove the `#[ignore]`s.
fn get_branches_pointed_at(_repository: &Path, _commitish: &str) -> Option<Vec<String>> {
    unimplemented!("corvene_git has no getBranchesPointedAt")
}

// GHD: unit/git/branch-test.ts › git/branch › tip › returns unborn for new repository
#[test]
fn returns_unborn_for_new_repository() {
    let repository = setup_empty_repository();

    let tip = load_tip(repository.path());

    let Tip::Unborn { name } = tip else {
        panic!("expected an unborn tip, got {tip:?}");
    };
    assert_eq!(name, "master");
}

// GHD: unit/git/branch-test.ts › git/branch › tip › returns correct ref if checkout occurs
#[test]
fn returns_correct_ref_if_checkout_occurs() {
    let repository = setup_empty_repository();

    exec(["checkout", "-b", "not-master"], repository.path());

    let tip = load_tip(repository.path());

    let Tip::Unborn { name } = tip else {
        panic!("expected an unborn tip, got {tip:?}");
    };
    assert_eq!(name, "not-master");
}

// GHD: unit/git/branch-test.ts › git/branch › tip › returns detached for arbitrary checkout
#[test]
fn returns_detached_for_arbitrary_checkout() {
    let repository = setup_fixture_repository("detached-head");

    let tip = load_tip(repository.path());

    let Tip::Detached { sha } = tip else {
        panic!("expected a detached tip, got {tip:?}");
    };
    assert_eq!(sha, "2acb028231d408aaa865f9538b1c89de5a2b9da8");
}

// GHD: unit/git/branch-test.ts › git/branch › tip › returns current branch when on a valid HEAD
#[test]
fn returns_current_branch_when_on_a_valid_head() {
    let repository = setup_fixture_repository("repo-with-many-refs");

    let tip = load_tip(repository.path());

    let Tip::Valid { branch } = tip else {
        panic!("expected a valid tip, got {tip:?}");
    };
    assert_eq!(branch.name, "commit-with-long-description");
    assert_eq!(
        branch.tip.as_deref(),
        Some("dfa96676b65e1c0ed43ca25492252a5e384c8efd")
    );
}

// GHD: unit/git/branch-test.ts › git/branch › tip › returns non-origin remote
#[test]
fn returns_non_origin_remote() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");

    let tip = load_tip(repository.path());

    let Tip::Valid { branch } = tip else {
        panic!("expected a valid tip, got {tip:?}");
    };
    assert_eq!(branch.upstream_remote_name(), Some("bassoon"));
}

// GHD: unit/git/branch-test.ts › git/branch › upstreamWithoutRemote › returns the upstream name without the remote prefix
#[test]
#[ignore = "ghd: missing: corvene_models::Branch has no upstreamWithoutRemote (models/branch.ts)"]
fn returns_the_upstream_name_without_the_remote_prefix() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");

    let tip = load_tip(repository.path());

    let Tip::Valid { branch } = tip else {
        panic!("expected a valid tip, got {tip:?}");
    };
    assert_eq!(branch.upstream_remote_name(), Some("bassoon"));
    assert_eq!(branch.upstream_short(), Some("bassoon/master"));
    assert_eq!(upstream_without_remote(&branch).as_deref(), Some("master"));
}

// GHD: unit/git/branch-test.ts › git/branch › getBranchesPointedAt › in a local repo › finds one branch name
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchesPointedAt (lib/git/branch.ts)"]
fn finds_one_branch_name() {
    let repository = setup_fixture_repository("test-repo");

    let branches = get_branches_pointed_at(repository.path(), "HEAD");
    let branches = branches.expect("branches !== null");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0], "master");
}

// GHD: unit/git/branch-test.ts › git/branch › getBranchesPointedAt › in a local repo › finds no branch names
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchesPointedAt (lib/git/branch.ts)"]
fn finds_no_branch_names() {
    let repository = setup_fixture_repository("test-repo");

    let branches = get_branches_pointed_at(repository.path(), "HEAD^");
    let branches = branches.expect("branches !== null");
    assert_eq!(branches.len(), 0);
}

// GHD: unit/git/branch-test.ts › git/branch › getBranchesPointedAt › in a local repo › returns null on a malformed committish
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchesPointedAt (lib/git/branch.ts)"]
fn returns_null_on_a_malformed_committish() {
    let repository = setup_fixture_repository("test-repo");

    let branches = get_branches_pointed_at(repository.path(), "MERGE_HEAD");
    assert!(branches.is_none());
}

// GHD: unit/git/branch-test.ts › git/branch › getBranchesPointedAt › in a repo with identical branches › finds multiple branch names
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchesPointedAt (lib/git/branch.ts)"]
fn finds_multiple_branch_names() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");
    create_branch(git(), repository.path(), "other-branch", None, false).expect("createBranch");

    let branches = get_branches_pointed_at(repository.path(), "HEAD");
    let branches = branches.expect("branches !== null");
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().any(|b| b == "other-branch"));
    assert!(branches.iter().any(|b| b == "master"));
}

// GHD: unit/git/branch-test.ts › git/branch › deleteLocalBranch › deletes local branches
#[test]
fn deletes_local_branches() {
    let repository = setup_fixture_repository("test-repo");

    let name = "test-branch";
    create_branch(git(), repository.path(), name, None, false).expect("createBranch");
    let branch = get_branches(repository.path(), &[&format!("refs/heads/{name}")])
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Could not create branch {name}"));

    let reference = format!("refs/heads/{name}");

    assert_eq!(get_branches(repository.path(), &[&reference]).len(), 1);

    delete_local_branch(git(), repository.path(), &branch.name).expect("deleteLocalBranch");

    assert_eq!(get_branches(repository.path(), &[&reference]).len(), 0);
}

/// `localBranch.upstreamRemoteName` and `localBranch.upstreamWithoutRemote`
/// as `Dispatcher::delete_branch` derives them for
/// `corvene_git::delete_remote_branch`.
fn delete_remote_branch_arguments(branch: &Branch) -> (Option<&str>, Option<&str>) {
    let remote = branch.upstream_remote_name();
    let remote_branch = branch
        .upstream_short()
        .and_then(|upstream| upstream.split_once('/'))
        .map(|(_, name)| name);
    (remote, remote_branch)
}

// GHD: unit/git/branch-test.ts › git/branch › deleteRemoteBranch › delete a local branches upstream branch
#[test]
fn delete_a_local_branches_upstream_branch() {
    let mock_remote = setup_fixture_repository("test-repo");

    let name = "test-branch";
    create_branch(git(), mock_remote.path(), name, None, false).expect("createBranch");
    let local_ref = format!("refs/heads/{name}");

    let mock_local = setup_local_fork_of_repository(&mock_remote);

    let remote_ref = format!("refs/remotes/origin/{name}");
    let remote_branch = get_branches(mock_local.path(), &[&remote_ref])
        .into_iter()
        .next()
        .expect("remoteBranch !== undefined");

    checkout_branch(git(), mock_local.path(), &remote_branch).expect("checkoutBranch");
    exec_ok(["checkout", "-"], mock_local.path());

    assert_eq!(get_branches(mock_local.path(), &[&local_ref]).len(), 1);
    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 1);

    let local_branch = get_branches(mock_local.path(), &[&local_ref])
        .into_iter()
        .next()
        .expect("localBranch !== undefined");
    let (upstream_remote_name, upstream_without_remote) =
        delete_remote_branch_arguments(&local_branch);
    let upstream_remote_name = upstream_remote_name.expect("upstreamRemoteName !== null");
    let upstream_without_remote = upstream_without_remote.expect("upstreamWithoutRemote !== null");

    delete_remote_branch(
        git(),
        mock_local.path(),
        upstream_remote_name,
        upstream_without_remote,
    )
    .expect("deleteRemoteBranch");

    assert_eq!(get_branches(mock_local.path(), &[&local_ref]).len(), 1);
    assert_eq!(get_branches(mock_local.path(), &[&remote_ref]).len(), 0);
    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 0);
}

// GHD: unit/git/branch-test.ts › git/branch › deleteRemoteBranch › handles attempted delete of removed remote branch
#[test]
#[ignore = "ghd: bug: delete_remote_branch fails (push: remote ref does not exist) and keeps refs/remotes/origin/test-branch; GHD treats BranchDeletionFailed as done and deletes that ref"]
fn handles_attempted_delete_of_removed_remote_branch() {
    let mock_remote = setup_fixture_repository("test-repo");

    let name = "test-branch";
    create_branch(git(), mock_remote.path(), name, None, false).expect("createBranch");
    let local_ref = format!("refs/heads/{name}");

    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 1);

    let mock_local = setup_local_fork_of_repository(&mock_remote);

    let remote_ref = format!("refs/remotes/origin/{name}");
    let remote_branch = get_branches(mock_local.path(), &[&remote_ref])
        .into_iter()
        .next()
        .expect("remoteBranch !== undefined");

    checkout_branch(git(), mock_local.path(), &remote_branch).expect("checkoutBranch");
    exec_ok(["checkout", "-"], mock_local.path());

    assert_eq!(get_branches(mock_local.path(), &[&local_ref]).len(), 1);
    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 1);

    let upstream_branch = get_branches(mock_remote.path(), &[&local_ref])
        .into_iter()
        .next()
        .expect("upstreamBranch !== undefined");
    delete_local_branch(git(), mock_remote.path(), &upstream_branch.name)
        .expect("deleteLocalBranch");
    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 0);

    let local_branch = get_branches(mock_local.path(), &[&local_ref])
        .into_iter()
        .next()
        .expect("localBranch !== undefined");
    let (upstream_remote_name, upstream_without_remote) =
        delete_remote_branch_arguments(&local_branch);
    let upstream_remote_name = upstream_remote_name.expect("upstreamRemoteName !== null");
    let upstream_without_remote = upstream_without_remote.expect("upstreamWithoutRemote !== null");

    delete_remote_branch(
        git(),
        mock_local.path(),
        upstream_remote_name,
        upstream_without_remote,
    )
    .expect("deleteRemoteBranch");

    assert_eq!(get_branches(mock_local.path(), &[&remote_ref]).len(), 0);
    assert_eq!(get_branches(mock_remote.path(), &[&local_ref]).len(), 0);
}
