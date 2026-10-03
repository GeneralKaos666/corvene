//! Port of GitHub Desktop's `app/test/unit/branch-pruner-test.ts`.
//!
//! Corvene has no branch pruner: nothing like GitHub Desktop's `BranchPruner`
//! (`lib/stores/helpers/branch-pruner.ts`, which deletes local branches
//! merged into the default branch whose upstream is gone, at most once a
//! day, sparing reserved names, branches checked out in the last two weeks
//! and branches checked out in a linked worktree) exists in `corvene-core`,
//! and neither do its building blocks `getMergedBranches`,
//! `getBranchCheckouts` or the last-prune date `RepositoriesStore` keeps
//! per GitHub repository. The cases call the stand-in
//! [`branch_pruner_run_once`] and are ignored until the pruner exists.
//!
//! `helpers/repository-builder-branch-pruner.ts` `setupRepository` is
//! [`setup_repository`]: the `corvene_models::Repository` of the path, with
//! the API repository it upserts as `Repository::github` when
//! `includesGhRepo`, the branches, remotes and tip `primeCaches` loads
//! through a `GitStore` (Corvene's refresh: `corvene_git::open_repository`),
//! and the last prune date, kept only for a GitHub repository as GitHub
//! Desktop's `updateLastPruneDate` does. `createRepository` is
//! `corvene_test_support::repository_builder_branch_pruner::create_repository`.
//! `offsetFromNow(n, unit)` is `SystemTime::now()` moved by `n` units.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use corvene_core::{GitHubRepository, Repository, RepositoryInfo, RepositoryPermission};
use corvene_test_support::repository_builder_branch_pruner::create_repository as create_pruned_repository;
use corvene_test_support::{TestRepo, exec, setup_fixture_repository};

const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The state GitHub Desktop's `setupRepository` leaves in the
/// `RepositoriesStore` and `RepositoryStateCache` for one repository.
#[allow(dead_code)]
struct PrunerRepository {
    repository: Repository,
    /// `branchesState` as `primeCaches` fills it.
    info: RepositoryInfo,
    /// `RepositoriesStore.getLastPruneDate(repository)`.
    last_prune_date: Option<SystemTime>,
}

/// GitHub Desktop's `setupRepository(path, repositoriesStore,
/// repositoriesStateCache, includesGhRepo, defaultBranchName,
/// lastPruneDate)`.
fn setup_repository(
    path: &TestRepo,
    includes_gh_repo: bool,
    default_branch_name: &str,
    last_prune_date: Option<SystemTime>,
) -> PrunerRepository {
    let mut repository = path.model();
    if includes_gh_repo {
        // the `IAPIFullRepository` upserted for `getDotComAPIEndpoint()`
        repository.github = Some(GitHubRepository {
            endpoint: "https://api.github.com".to_string(),
            owner: String::new(),
            name: "string".to_string(),
            html_url: "string".to_string(),
            clone_url: "string".to_string(),
            default_branch: Some(default_branch_name.to_string()),
            private: false,
            fork: false,
            parent: None,
            archived: false,
            // pull: true, push: true, admin: false
            permissions: Some(RepositoryPermission::Write),
            allow_forking: None,
        });
    }
    // `primeCaches`: `loadRemotes`, `loadBranches`, `loadStatus`
    let info = corvene_git::open_repository(path.path())
        .unwrap_or_else(|err| panic!("open {}: {err}", path.path().display()));
    let last_prune_date = last_prune_date.filter(|_| repository.github.is_some());
    PrunerRepository {
        repository,
        info,
        last_prune_date,
    }
}

/// Stand-in for `new BranchPruner(repository, gitStoreCache,
/// repositoriesStore, repositoriesStateCache, onPruneCompleted).runOnce()`
/// (`lib/stores/helpers/branch-pruner.ts`). Replace it with the Corvene
/// pruner once there is one and remove the `#[ignore]`s.
fn branch_pruner_run_once(_repository: &PrunerRepository) {
    unimplemented!("Corvene has no BranchPruner")
}

/// `getBranchesFromGit(repository)`: the names `git branch` lists.
fn get_branches_from_git(repository: &Path) -> Vec<String> {
    let git_output = exec(["branch"], repository);
    git_output
        .stdout
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s[2..].to_string())
        .collect()
}

/// Removes the linked worktree GitHub Desktop's last case creates next to
/// the repository (outside its temporary directory).
struct RemoveDirOnDrop(PathBuf);

impl Drop for RemoveDirOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › does nothing on non GitHub repositories
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn does_nothing_on_non_git_hub_repositories() {
    let path = setup_fixture_repository("branch-prune-tests");

    let repo = setup_repository(&path, false, "master", None);

    let branches_before_pruning = get_branches_from_git(repo.repository.path.as_path());
    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert_eq!(branches_before_pruning, branches_after_pruning);
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › prunes for GitHub repository
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn prunes_for_git_hub_repository() {
    let last_prune_date = SystemTime::now() - DAY;

    let path = setup_fixture_repository("branch-prune-tests");
    let repo = setup_repository(&path, true, "master", Some(last_prune_date));

    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert!(
        !branches_after_pruning
            .iter()
            .any(|b| b == "deleted-branch-1")
    );
    assert!(
        branches_after_pruning
            .iter()
            .any(|b| b == "not-deleted-branch-1")
    );
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › does not prune if the last prune date is less than 24 hours ago
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn does_not_prune_if_the_last_prune_date_is_less_than_24_hours_ago() {
    let last_prune_date = SystemTime::now() - 4 * HOUR;
    let path = setup_fixture_repository("branch-prune-tests");
    let repo = setup_repository(&path, true, "master", Some(last_prune_date));

    let branches_before_pruning = get_branches_from_git(repo.repository.path.as_path());
    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert_eq!(branches_before_pruning, branches_after_pruning);
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › does not prune if there is no default branch
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn does_not_prune_if_there_is_no_default_branch() {
    let last_prune_date = SystemTime::now() - DAY;
    let repo_path = setup_fixture_repository("branch-prune-tests");
    fs::remove_file(
        repo_path
            .path()
            .join(".git")
            .join("refs")
            .join("remotes")
            .join("origin")
            .join("HEAD"),
    )
    .expect("unlink refs/remotes/origin/HEAD");

    let repo = setup_repository(&repo_path, true, "", Some(last_prune_date));

    let branches_before_pruning = get_branches_from_git(repo.repository.path.as_path());
    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert_eq!(branches_before_pruning, branches_after_pruning);
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › does not prune reserved branches
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn does_not_prune_reserved_branches() {
    let last_prune_date = SystemTime::now() - DAY;

    let path = setup_fixture_repository("branch-prune-tests");
    let repo = setup_repository(&path, true, "master", Some(last_prune_date));

    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    let expected_branches_after_pruning = [
        "master",
        "gh-pages",
        "develop",
        "dev",
        "development",
        "trunk",
        "devel",
        "release",
    ];

    for branch in expected_branches_after_pruning {
        assert!(branches_after_pruning.iter().any(|b| b == branch));
    }
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › never prunes a branch that lacks an upstream
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn never_prunes_a_branch_that_lacks_an_upstream() {
    let path = create_pruned_repository();

    let last_prune_date = SystemTime::now() - DAY;

    let repo = setup_repository(&path, true, "master", Some(last_prune_date));

    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert!(branches_after_pruning.iter().any(|b| b == "master"));
    assert!(branches_after_pruning.iter().any(|b| b == "other-branch"));
}

// GHD: unit/branch-pruner-test.ts › BranchPruner › does not prune branches checked out in a linked worktree
#[test]
#[ignore = "ghd: missing: Corvene has no BranchPruner (lib/stores/helpers/branch-pruner.ts) nor getMergedBranches / getBranchCheckouts / a last prune date"]
fn does_not_prune_branches_checked_out_in_a_linked_worktree() {
    let last_prune_date = SystemTime::now() - DAY;

    let repo_path = setup_fixture_repository("branch-prune-tests");

    // Create a linked worktree with `deleted-branch-1` checked out.
    // This branch would normally be pruned (merged, upstream gone),
    // but the worktree checkout should protect it.
    let worktree_path = PathBuf::from(format!("{}-worktree", repo_path.path().display()));
    let _worktree = RemoveDirOnDrop(worktree_path.clone());
    exec(
        [
            OsStr::new("worktree"),
            OsStr::new("add"),
            worktree_path.as_os_str(),
            OsStr::new("deleted-branch-1"),
        ],
        repo_path.path(),
    );

    let repo = setup_repository(&repo_path, true, "master", Some(last_prune_date));

    branch_pruner_run_once(&repo);
    let branches_after_pruning = get_branches_from_git(repo.repository.path.as_path());

    assert!(
        branches_after_pruning
            .iter()
            .any(|b| b == "deleted-branch-1"),
        "expected deleted-branch-1 to be preserved because it is checked out in a linked worktree"
    );
}
