//! Port of GitHub Desktop's `app/test/unit/git/reset-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `reset(repository, mode, ref)` (`lib/git/reset.ts`) is
//!   `corvene_git::reset_to(git, path, mode, ref)`; `GitResetMode` is
//!   `corvene_git::ResetMode`.
//! - `resetPaths(repository, mode, ref, paths)` has no equivalent: Corvene
//!   only resets every path (`git reset -- .` in `unstage_all` and
//!   `discard_changes`). The case calls the stand-in [`reset_paths`] and is
//!   ignored until it exists. GitHub Desktop itself marks this case
//!   `it.skip`; its body awaits nothing between `git add` and the unlink,
//!   which a blocking port does not have to work around.

use std::path::Path;

use corvene_git::ResetMode;
use corvene_test_support::{TestRepo, exec, get_status_or_throw, git, setup_fixture_repository};

/// Stand-in for GitHub Desktop's `resetPaths(repository, mode, ref, paths)`
/// (`lib/git/reset.ts`): `git reset [--hard | --soft] <ref> -- <paths>`
/// (`--stdin -z` for a mixed reset on Windows). Replace it with the
/// `corvene_git` function once there is one and remove the `#[ignore]`.
fn reset_paths(_repository: &TestRepo, _mode: ResetMode, _reference: &str, _paths: &[&Path]) {
    unimplemented!("corvene_git has no resetPaths")
}

// GHD: unit/git/reset-test.ts › git/reset › reset › can hard reset a repository
#[test]
fn can_hard_reset_a_repository() {
    let repository = setup_fixture_repository("test-repo");

    let repo_path = repository.path();
    let file_name = "README.md";
    let file_path = repo_path.join(file_name);

    std::fs::write(&file_path, "Hi world\n").unwrap();

    corvene_git::reset_to(git(), repository.path(), ResetMode::Hard, "HEAD").expect("reset");

    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 0);
}

// GHD: unit/git/reset-test.ts › git/reset › resetPaths › resets discarded staged file
#[test]
#[ignore = "ghd: missing: corvene_git has no resetPaths (lib/git/reset.ts), only whole-index resets"]
fn resets_discarded_staged_file() {
    let repository = setup_fixture_repository("test-repo");

    let repo_path = repository.path();
    let file_name = "README.md";
    let file_path = repo_path.join(file_name);

    // modify the file
    std::fs::write(&file_path, "Hi world\n").unwrap();

    // stage the file, then delete it to mimic discarding
    exec(["add", file_name], repo_path);
    std::fs::remove_file(&file_path).unwrap();

    reset_paths(&repository, ResetMode::Mixed, "HEAD", &[&file_path]);

    // then checkout the version from the index to restore it
    exec(
        ["checkout-index", "-f", "-u", "-q", "--", file_name],
        repo_path,
    );

    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 0);
}
