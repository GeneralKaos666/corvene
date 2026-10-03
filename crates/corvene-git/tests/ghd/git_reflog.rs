//! Port of GitHub Desktop's `app/test/unit/git/reflog-test.ts`.
//!
//! Corvene equivalents (`lib/git/reflog.ts`):
//!
//! - `getRecentBranches(repository, limit)`: `corvene_git::recent_branches`
//!   (the same `git log -g` over HEAD's reflog, parsed by
//!   `parse_recent_branches`).
//! - `getBranchCheckouts(repository, afterDate)`: none; GitHub Desktop uses
//!   it for the branch pruner, which Corvene does not have.
//!   [`get_branch_checkouts`] is a stand-in.
//! - `createBranch`, `checkoutBranch`, `renameBranch`:
//!   `corvene_git::{create_branch, checkout_branch, rename_branch}`
//!   (`renameBranch` passes `branch.nameWithoutRemote`, Corvene's
//!   `Branch::name_without_remote`); `getBranches`: [`get_branches`].
//!   `offsetFromNow(n, unit)` is `SystemTime::now()` moved by `n` units.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use corvene_git::{checkout_branch, create_branch, recent_branches, rename_branch};
use corvene_test_support::{exec, get_branches, git, setup_fixture_repository};

fn create_and_checkout(repository: &Path, name: &str) {
    create_branch(git(), repository, name, None, false).expect("createBranch");
    let branch = get_branches(repository, &[&format!("refs/heads/{name}")])
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Unable to create branch: {name}"));
    checkout_branch(git(), repository, &branch).expect("checkoutBranch");
}

/// Stand-in for GitHub Desktop's `getBranchCheckouts(repository,
/// afterDate)`: the branches checked out on or after `after_date` (from
/// HEAD's reflog), with the date of their latest checkout. Replace it with
/// the `corvene_git` function once there is one and remove the
/// `#[ignore]`s.
fn get_branch_checkouts(
    _repository: &Path,
    _after_date: SystemTime,
) -> HashMap<String, SystemTime> {
    unimplemented!("corvene_git has no getBranchCheckouts")
}

const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

// GHD: unit/git/reflog-test.ts › git/reflog › getRecentBranches › returns the recently checked out branches
#[test]
fn returns_the_recently_checked_out_branches() {
    let repository = setup_fixture_repository("test-repo");

    create_and_checkout(repository.path(), "branch-1");
    create_and_checkout(repository.path(), "branch-2");

    let branches = recent_branches(git(), repository.path(), 10).expect("getRecentBranches");
    assert!(branches.iter().any(|b| b == "branch-1"));
    assert!(branches.iter().any(|b| b == "branch-2"));
}

// GHD: unit/git/reflog-test.ts › git/reflog › getRecentBranches › works after renaming a branch
#[test]
fn works_after_renaming_a_branch() {
    let repository = setup_fixture_repository("test-repo");

    create_and_checkout(repository.path(), "branch-1");
    create_and_checkout(repository.path(), "branch-2");

    let all_branches = get_branches(repository.path(), &[]);
    let current_branch = all_branches.iter().find(|branch| branch.name == "branch-2");

    let current_branch = current_branch.expect("currentBranch !== undefined");
    rename_branch(
        git(),
        repository.path(),
        current_branch.name_without_remote(),
        "branch-2-test",
    )
    .expect("renameBranch");

    let branches = recent_branches(git(), repository.path(), 10).expect("getRecentBranches");
    assert!(!branches.iter().any(|b| b == "branch-2"));
    assert!(branches.iter().any(|b| b == "branch-1"));
    assert!(branches.iter().any(|b| b == "branch-2-test"));
}

// GHD: unit/git/reflog-test.ts › git/reflog › getRecentBranches › returns a limited number of branches
#[test]
fn returns_a_limited_number_of_branches() {
    let repository = setup_fixture_repository("test-repo");

    create_and_checkout(repository.path(), "branch-1");
    create_and_checkout(repository.path(), "branch-2");
    create_and_checkout(repository.path(), "branch-3");
    create_and_checkout(repository.path(), "branch-4");

    let branches = recent_branches(git(), repository.path(), 2).expect("getRecentBranches");
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().any(|b| b == "branch-4"));
    assert!(branches.iter().any(|b| b == "branch-3"));
}

// GHD: unit/git/reflog-test.ts › git/reflog › getBranchCheckouts › returns does not return the branches that were checked out before a specific date
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchCheckouts (lib/git/reflog.ts)"]
fn returns_does_not_return_the_branches_checked_out_before_a_date() {
    let repository = setup_fixture_repository("test-repo");

    create_and_checkout(repository.path(), "branch-1");
    create_and_checkout(repository.path(), "branch-2");

    let branches = get_branch_checkouts(repository.path(), SystemTime::now() + DAY);
    assert_eq!(branches.len(), 0);
}

// GHD: unit/git/reflog-test.ts › git/reflog › getBranchCheckouts › returns all branches checked out after a specific date
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchCheckouts (lib/git/reflog.ts)"]
fn returns_all_branches_checked_out_after_a_specific_date() {
    let repository = setup_fixture_repository("test-repo");

    create_branch(git(), repository.path(), "never-checked-out", None, false)
        .expect("createBranch");
    create_and_checkout(repository.path(), "branch-1");
    create_and_checkout(repository.path(), "branch-2");

    let branches = get_branch_checkouts(repository.path(), SystemTime::now() - HOUR);
    assert_eq!(branches.len(), 2);
}

// GHD: unit/git/reflog-test.ts › git/reflog › getBranchCheckouts › returns empty when current branch is orphaned
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchCheckouts (lib/git/reflog.ts)"]
fn returns_empty_when_current_branch_is_orphaned() {
    let repository = setup_fixture_repository("test-repo");

    let result = exec(["checkout", "--orphan", "orphan-branch"], repository.path());
    assert_eq!(result.exit_code, 0);

    let branches = get_branch_checkouts(repository.path(), SystemTime::now() - HOUR);
    assert_eq!(branches.len(), 0);
}
