//! Port of GitHub Desktop's `app/test/unit/git/submodule-test.ts`.
//!
//! Corvene has no equivalent of `listSubmodules` (`git submodule status`,
//! used by GitHub Desktop's `GitStore.discardChanges` to tell submodules
//! apart) or `resetSubmodulePaths` (`git submodule update --recursive
//! --force -- <paths>`, which GitHub Desktop's discard runs for submodule
//! entries) in `lib/git/submodule.ts`: Corvene's `corvene_git::discard_changes`
//! never resets a submodule's commit, and only with flag
//! `720-discard-submodule-changes` (off in the GitHub Desktop preset) runs
//! `git checkout -f -- .` inside a submodule with modified files, so in the
//! GitHub Desktop preset a discarded submodule keeps both its commit and its
//! dirty files (`.docs/deviations.md` says GitHub Desktop "leaves it dirty",
//! but its `--force` update does revert tracked files, which the last case
//! checks). Neither is a counterpart of these functions: the cases call stand-ins
//! ([`list_submodules`], [`reset_submodule_paths`]) and are ignored until
//! they exist. The setup runs Corvene's own calls: `getBranches` is
//! [`get_branches`], `checkoutBranch` is `corvene_git::checkout_branch`.

use std::path::Path;

use corvene_test_support::{TestRepo, get_branches, git, setup_fixture_repository};

/// GitHub Desktop's `SubmoduleEntry` (`models/submodule.ts`).
#[derive(Debug)]
struct SubmoduleEntry {
    sha: String,
    path: String,
    describe: String,
}

/// Stand-in for GitHub Desktop's `listSubmodules(repository)`
/// (`lib/git/submodule.ts`). Replace this with the `corvene_git` function
/// once there is one and remove the `#[ignore]`s.
fn list_submodules(_repository: &TestRepo) -> Vec<SubmoduleEntry> {
    unimplemented!("corvene_git has no listSubmodules")
}

/// Stand-in for GitHub Desktop's `resetSubmodulePaths(repository, paths)`
/// (`lib/git/submodule.ts`). Replace this with the `corvene_git` function
/// once there is one and remove the `#[ignore]`s.
fn reset_submodule_paths(_repository: &TestRepo, _paths: &[&str]) {
    unimplemented!("corvene_git has no resetSubmodulePaths")
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
#[ignore = "ghd: missing: corvene_git has no listSubmodules (lib/git/submodule.ts)"]
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
#[ignore = "ghd: missing: corvene_git has no listSubmodules (lib/git/submodule.ts)"]
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
#[ignore = "ghd: missing: corvene_git has no listSubmodules / resetSubmodulePaths (lib/git/submodule.ts)"]
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
#[ignore = "ghd: missing: corvene_git has no resetSubmodulePaths (lib/git/submodule.ts)"]
fn eliminate_submodule_dirty_state() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let submodule_path = repository.join("foo").join("submodule");

    let file_path = submodule_path.join("README.md");
    std::fs::write(&file_path, "changed").unwrap();

    reset_submodule_paths(&repository, &["foo/submodule"]);

    let result = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(result, "# submodule-test-case");
}
