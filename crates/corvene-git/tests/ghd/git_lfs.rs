//! Port of GitHub Desktop's `app/test/unit/git/lfs-test.ts`.
//!
//! - `isUsingLFS(repository)` (`lib/git/lfs.ts`) is
//!   `corvene_git::is_using_lfs` (`git lfs track --json`). Corvene decides
//!   whether to offer Initialize Git LFS with `is_using_lfs_by_attributes`
//!   instead while flag `905-lfs-detect-by-attributes` is on; its GitHub
//!   Desktop value (off) uses `is_using_lfs`, which is what these cases
//!   test.
//! - `isTrackedByLFS` and `filesNotTrackedByLFS` (same file, behind GitHub
//!   Desktop's warning about files over 100 MB that LFS does not track) are
//!   `corvene_git::is_tracked_by_lfs` and `files_not_tracked_by_lfs`.
//!
//! GitHub Desktop runs with dugite's bundled git, which always has Git LFS.
//! Every case that runs `git lfs` (through `exec(['lfs', 'track', …])` or
//! through `isUsingLFS` itself) needs `git-lfs` on the git found by
//! `corvene_test_support::git()`, and first calls [`require_git_lfs`], which
//! fails the case when `git lfs` cannot run. Without that check the two
//! "returns false" cases of `isUsingLFS` would pass without exercising LFS
//! (`is_using_lfs` treats a failing `git lfs` as "not using LFS"), and
//! "returns files not listed in Git LFS" would test a repository with no
//! LFS rule at all. Those cases are ignored as `env` while the test git
//! has no `git-lfs`; with `git-lfs` on `PATH` they pass.

use std::path::Path;

use corvene_git::error::Result;
use corvene_git::is_using_lfs;
use corvene_test_support::{exec, git, setup_empty_repository, setup_fixture_repository};

/// The environment GitHub Desktop's tests take for granted: `git lfs` runs
/// (dugite's git bundles Git LFS). Fails the case with a clear message when
/// the test git has no `git-lfs`, instead of letting it pass or fail
/// without LFS ever running.
fn require_git_lfs(cwd: &Path) {
    let result = exec(["lfs", "version"], cwd);
    assert_eq!(
        result.exit_code,
        0,
        "git-lfs is not available to the test git (GitHub Desktop's dugite git bundles it): {}",
        result.stderr.trim()
    );
}

/// GitHub Desktop's `isTrackedByLFS(repository, path)`.
fn is_tracked_by_lfs(repository: &Path, path: &str) -> Result<bool> {
    corvene_git::is_tracked_by_lfs(git(), repository, path)
}

/// GitHub Desktop's `filesNotTrackedByLFS(repository, filePaths)`.
fn files_not_tracked_by_lfs(repository: &Path, file_paths: &[&str]) -> Result<Vec<String>> {
    corvene_git::files_not_tracked_by_lfs(git(), repository, file_paths)
}

// GHD: unit/git/lfs-test.ts › git-lfs › isUsingLFS › returns false for repository not using LFS
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn is_using_lfs_returns_false_for_repository_not_using_lfs() {
    let repository = setup_fixture_repository("test-repo");
    require_git_lfs(repository.path());

    let using_lfs = is_using_lfs(git(), repository.path());
    assert!(!using_lfs);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isUsingLFS › returns true if LFS is tracking a path
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_true_if_lfs_is_tracking_a_path() {
    let repository = setup_fixture_repository("test-repo");

    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.psd"], repository.path());

    let using_lfs = is_using_lfs(git(), repository.path());
    assert!(using_lfs);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isUsingLFS › returns false if a non-LFS Git filter is configured
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_false_if_a_non_lfs_git_filter_is_configured() {
    let repository = setup_empty_repository();
    let attributes_path = repository.join(".git").join("info").join("attributes");

    std::fs::write(&attributes_path, "* filter=annex\n").unwrap();

    require_git_lfs(repository.path());
    let using_lfs = is_using_lfs(git(), repository.path());
    assert!(!using_lfs);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isUsingLFS › returns true if LFS tracks a path alongside a non-LFS Git filter
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_true_if_lfs_tracks_a_path_alongside_a_non_lfs_git_filter() {
    let repository = setup_empty_repository();
    let attributes_path = repository.join(".git").join("info").join("attributes");

    std::fs::write(&attributes_path, "* filter=annex\n").unwrap();
    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.psd"], repository.path());

    let using_lfs = is_using_lfs(git(), repository.path());
    assert!(using_lfs);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isTrackedByLFS › returns false for repository not using LFS
#[test]
fn is_tracked_by_lfs_returns_false_for_repository_not_using_lfs() {
    let repository = setup_empty_repository();

    let file = "README.md";
    let readme = repository.join(file);
    std::fs::write(&readme, "Hello world!").unwrap();

    let found = is_tracked_by_lfs(repository.path(), file).unwrap();
    assert!(!found);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isTrackedByLFS › returns true after tracking file in Git LFS
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_true_after_tracking_file_in_git_lfs() {
    let repository = setup_empty_repository();

    let file = "README.md";
    let readme = repository.join(file);
    std::fs::write(&readme, "Hello world!").unwrap();

    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.md"], repository.path());

    let found = is_tracked_by_lfs(repository.path(), file).unwrap();
    assert!(found);
}

// GHD: unit/git/lfs-test.ts › git-lfs › isTrackedByLFS › returns true after tracking file with character issues in Git LFS
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_true_after_tracking_file_with_character_issues_in_git_lfs() {
    let repository = setup_empty_repository();

    let file = "Top Ten Worst Repositories to host on GitHub - Carlos Martín Nieto.md";
    let readme = repository.join(file);
    std::fs::write(&readme, "Hello world!").unwrap();

    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.md"], repository.path());

    let found = is_tracked_by_lfs(repository.path(), file).unwrap();
    assert!(found);
}

// GHD: unit/git/lfs-test.ts › git-lfs › filesNotTrackedByLFS › returns files not listed in Git LFS
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn returns_files_not_listed_in_git_lfs() {
    let repository = setup_empty_repository();
    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.md"], repository.path());

    let video_file = "some-video-file.mp4";

    let not_found = files_not_tracked_by_lfs(repository.path(), &[video_file]).unwrap();

    assert_eq!(not_found.len(), 1);
    assert!(not_found.iter().any(|f| f == video_file));
}

// GHD: unit/git/lfs-test.ts › git-lfs › filesNotTrackedByLFS › skips files that are tracked by Git LFS
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn skips_files_that_are_tracked_by_git_lfs() {
    let repository = setup_empty_repository();
    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.png"], repository.path());

    let photo_file = "some-cool-photo.png";

    let not_found = files_not_tracked_by_lfs(repository.path(), &[photo_file]).unwrap();

    assert_eq!(not_found.len(), 0);
}

// GHD: unit/git/lfs-test.ts › git-lfs › filesNotTrackedByLFS › skips files in a subfolder that are tracked
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn skips_files_in_a_subfolder_that_are_tracked() {
    let repository = setup_empty_repository();
    require_git_lfs(repository.path());
    exec(["lfs", "track", "*.png"], repository.path());

    let photo_file_in_directory = "app/src/some-cool-photo.png";
    let not_found =
        files_not_tracked_by_lfs(repository.path(), &[photo_file_in_directory]).unwrap();

    assert_eq!(not_found.len(), 0);
}

// GHD: unit/git/lfs-test.ts › git-lfs › filesNotTrackedByLFS › skips files in a subfolder where the rule only covers the subdirectory
#[test]
#[ignore = "ghd: env: needs git-lfs, which the test git lacks (GHD's dugite git bundles it); passes with git-lfs on PATH"]
fn skips_files_in_a_subfolder_where_the_rule_only_covers_the_subdirectory() {
    let repository = setup_empty_repository();
    require_git_lfs(repository.path());
    exec(["lfs", "track", "app/src/*.png"], repository.path());

    let photo_file_in_directory = "app/src/some-cool-photo.png";
    let not_found =
        files_not_tracked_by_lfs(repository.path(), &[photo_file_in_directory]).unwrap();

    assert_eq!(not_found.len(), 0);
}
