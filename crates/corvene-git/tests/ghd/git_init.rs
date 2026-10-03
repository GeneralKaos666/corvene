//! Port of GitHub Desktop's `app/test/unit/git/init-test.ts`.
//!
//! GitHub Desktop's `initGitRepository(path)` (`lib/git/init.ts`) is `git
//! -c init.defaultBranch=<getDefaultBranch()> init`. Corvene runs
//! `corvene_git::init_repository` with the branch from
//! `corvene_git::configured_default_branch` (`getDefaultBranch`: the global
//! `init.defaultBranch`, else `main`), as `Dispatcher::create_repository`
//! does; [`init_git_repository`] is that call without the files the Create
//! Repository dialog adds afterwards.

use std::path::Path;

use corvene_git::{InitOptions, configured_default_branch, init_repository};
use corvene_test_support::{TestRepo, create_temp_directory, exec, git, lock_global_config};

/// GitHub Desktop's `initGitRepository(path)`.
fn init_git_repository(path: &Path) {
    init_repository(
        git(),
        InitOptions {
            path: path.to_path_buf(),
            default_branch: Some(get_default_branch()),
            description: None,
            readme: false,
            gitignore: None,
            license: None,
            git_attributes: None,
            keep_existing: false,
        },
    )
    .expect("init");
}

/// GitHub Desktop's `getDefaultBranch()` (`lib/helpers/default-branch.ts`).
fn get_default_branch() -> String {
    configured_default_branch(git())
}

// GHD: unit/git/init-test.ts › git/init › creates a new git repository
#[test]
fn creates_a_new_git_repository() {
    let temp_dir = create_temp_directory();
    init_git_repository(temp_dir.path());

    let git_dir = temp_dir.path().join(".git");
    assert!(git_dir.exists());
}

// GHD: unit/git/init-test.ts › git/init › creates a repository with a default branch
#[test]
fn creates_a_repository_with_a_default_branch() {
    // the default branch comes from the (shared) global configuration
    let _global = lock_global_config();
    let temp_dir = create_temp_directory();
    init_git_repository(temp_dir.path());

    let repo = TestRepo::from_temp_dir(temp_dir);
    // GitHub Desktop's `getStatus` returns null for a missing repository and
    // `exists: true` otherwise; Corvene's returns an error or the status
    let status = corvene_git::get_status(git(), repo.path(), None);
    assert!(status.is_ok(), "{status:?}");

    let head = exec(["symbolic-ref", "--short", "HEAD"], repo.path());
    assert_eq!(head.stdout.trim(), get_default_branch());
}
