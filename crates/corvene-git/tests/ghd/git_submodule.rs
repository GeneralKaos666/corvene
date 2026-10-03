//! Port of GitHub Desktop's `app/test/unit/git/submodule-test.ts`.
//!
//! `listSubmodules(repository)` and `resetSubmodulePaths(repository, paths)`
//! (`lib/git/submodule.ts`) are `corvene_git::list_submodules` and
//! `corvene_git::reset_submodule_paths`; GitHub Desktop's `SubmoduleEntry`
//! (`models/submodule.ts`) is `corvene_git::SubmoduleEntry`. Corvene's
//! discard (`corvene_git::discard_changes`) resets submodule entries with
//! `reset_submodule_paths`, as GitHub Desktop's `GitStore.discardChanges`
//! does, but tells them apart by their status rather than by
//! `listSubmodules`. The setup runs
//! Corvene's own calls: `getBranches` is [`get_branches`], `checkoutBranch`
//! is `corvene_git::checkout_branch`.

use std::path::Path;

use corvene_git::SubmoduleEntry;
use corvene_test_support::{TestRepo, get_branches, git, setup_fixture_repository};

/// `listSubmodules(repository)`.
fn list_submodules(repository: &TestRepo) -> Vec<SubmoduleEntry> {
    corvene_git::list_submodules(git(), repository.path()).expect("listSubmodules")
}

/// `resetSubmodulePaths(repository, paths)`.
fn reset_submodule_paths(repository: &TestRepo, paths: &[&str]) {
    corvene_git::reset_submodule_paths(git(), repository.path(), paths)
        .expect("resetSubmodulePaths");
}

/// `getBranches(submoduleRepository, 'refs/remotes/origin/feature-branch')`
/// and `checkoutBranch(submoduleRepository, branches[0], null)`.
fn checkout_feature_branch(submodule_path: &Path) {
    let branches = get_branches(submodule_path, &["refs/remotes/origin/feature-branch"]);

    let Some(branch) = branches.first() else {
        panic!("Could not find branch: feature-branch");
    };

    corvene_git::checkout_branch(git(), submodule_path, branch).expect("checkoutBranch");
}

// GHD: unit/git/submodule-test.ts › git/submodule › listSubmodules › returns the submodule entry
#[test]
fn returns_the_submodule_entry() {
    let repository = setup_fixture_repository("submodule-basic-setup");
    let result = list_submodules(&repository);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sha, "c59617b65080863c4ca72c1f191fa1b423b92223");
    assert_eq!(result[0].path, "foo/submodule");
    assert_eq!(result[0].describe, "first-tag~2");
}

// GHD: unit/git/submodule-test.ts › git/submodule › listSubmodules › returns the expected tag
#[test]
fn returns_the_expected_tag() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let submodule_path = repository.join("foo").join("submodule");

    checkout_feature_branch(&submodule_path);

    let result = list_submodules(&repository);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sha, "14425bb2a4ee361af7f789a81b971f8466ae521d");
    assert_eq!(result[0].path, "foo/submodule");
    assert_eq!(result[0].describe, "heads/feature-branch");
}

// GHD: unit/git/submodule-test.ts › git/submodule › resetSubmodulePaths › update submodule to original commit
#[test]
fn update_submodule_to_original_commit() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let submodule_path = repository.join("foo").join("submodule");

    checkout_feature_branch(&submodule_path);

    let result = list_submodules(&repository);
    assert_eq!(result[0].describe, "heads/feature-branch");

    reset_submodule_paths(&repository, &["foo/submodule"]);

    let result = list_submodules(&repository);
    assert_eq!(result[0].describe, "first-tag~2");
}

// GHD: unit/git/submodule-test.ts › git/submodule › resetSubmodulePaths › eliminate submodule dirty state
#[test]
fn eliminate_submodule_dirty_state() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let submodule_path = repository.join("foo").join("submodule");

    let file_path = submodule_path.join("README.md");
    std::fs::write(&file_path, "changed").unwrap();

    reset_submodule_paths(&repository, &["foo/submodule"]);

    let result = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(result, "# submodule-test-case");
}
