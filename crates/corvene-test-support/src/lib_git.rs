//! GitHub Desktop's `lib/git` functions as its tests call them, made of the
//! `corvene_git` calls Corvene uses for the same job. A function under test
//! is called directly in the test module; these are the ones tests use for
//! setup and checks.

use std::ffi::OsStr;
use std::path::Path;

use corvene_git::{CommitOptions, GitError};
use corvene_models::{Branch, Commit, Diff, Tip, WorkingDirectoryFileChange};
use tempfile::TempDir;

use crate::exec::{exec, git};
use crate::repositories::TestRepo;
use crate::temp::create_temp_directory;

/// GitHub Desktop's `getBranches(repository, ...prefixes)`
/// (`lib/git/for-each-ref.ts`): `git for-each-ref` over `refs/heads` and
/// `refs/remotes` (or the given prefixes), symbolic refs left out, and `[]`
/// when git reports `NotAGitRepository`.
///
/// Corvene reads the same branches with `corvene_git::open_repository`
/// (`RepositoryInfo::branches`, which leaves out `<remote>/HEAD`), as
/// [`get_branch_or_error`](crate::get_branch_or_error) does; its
/// `GitError::NotARepository` is GitHub Desktop's `NotAGitRepository`. A
/// prefix matches as a `for-each-ref` pattern does: the whole ref name, or
/// its leading components. No prefixes (`&[]`) is every branch.
///
/// # Panics
///
/// When the repository cannot be read for another reason (GitHub Desktop's
/// promise rejects).
pub fn get_branches(repository: &Path, prefixes: &[&str]) -> Vec<Branch> {
    let branches = match corvene_git::open_repository(repository) {
        Ok(info) => info.branches,
        Err(GitError::NotARepository(_)) => return Vec::new(),
        Err(err) => panic!("getBranches in {}: {err}", repository.display()),
    };
    branches
        .into_iter()
        .filter(|b| {
            prefixes.is_empty()
                || prefixes.iter().any(|p| {
                    let p = p.trim_end_matches('/');
                    b.full_name == p
                        || b.full_name
                            .strip_prefix(p)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
        })
        .collect()
}

/// GitHub Desktop's `createCommit(repository, message, files, options)`
/// (`lib/git/commit.ts`): `unstageAll`, `stageFiles(files)`, `git commit -F
/// -`. Corvene's `Dispatcher::commit` makes the same `corvene_git` calls
/// (`unstage_all`, `stage_files` and `stage_partial_files`, together GitHub
/// Desktop's `stageFiles`, then `commit`); this always unstages first, as
/// GitHub Desktop does (the dispatcher skips the reset when every file is
/// fully selected and nothing is conflicted). Returns what
/// `corvene_git::commit` returns; an `Err` is GitHub Desktop's rejection.
pub fn try_create_commit(
    repository: &TestRepo,
    message: &str,
    files: &[WorkingDirectoryFileChange],
    options: &CommitOptions,
) -> Result<String, GitError> {
    let path = repository.path();
    corvene_git::unstage_all(git(), path)?;
    corvene_git::stage_files(git(), path, files)?;
    corvene_git::stage_partial_files(git(), path, files)?;
    corvene_git::commit(git(), path, message, options)
}

/// GitHub Desktop's `createCommit(repository, message, files)` with no
/// options, as a setup step: [`try_create_commit`] that fails the test when
/// a git call fails, returning the new commit's sha.
///
/// # Panics
///
/// When any of the git calls fails (GitHub Desktop's promise rejects).
pub fn create_commit(
    repository: &TestRepo,
    message: &str,
    files: &[WorkingDirectoryFileChange],
) -> String {
    try_create_commit(repository, message, files, &CommitOptions::default())
        .unwrap_or_else(|err| panic!("createCommit in {}: {err}", repository.path().display()))
}

/// GitHub Desktop's `getCommits(repository, revisionRange, limit)`
/// (`lib/git/log.ts`): `corvene_git::get_commits(path, revisionRange, 0,
/// limit)`.
///
/// # Panics
///
/// When git fails.
pub fn get_commits(repository: &TestRepo, revision_range: &str, limit: usize) -> Vec<Commit> {
    corvene_git::get_commits(repository.path(), revision_range, 0, limit)
        .unwrap_or_else(|err| panic!("getCommits {revision_range}: {err}"))
}

/// GitHub Desktop's `getCommit(repository, ref)` (`lib/git/log.ts`): the
/// first commit `getCommits(repository, ref, 1)` returns (`null` is
/// `None`).
///
/// # Panics
///
/// When git fails.
pub fn get_commit(repository: &TestRepo, reference: &str) -> Option<Commit> {
    corvene_git::get_commits(repository.path(), reference, 0, 1)
        .unwrap_or_else(|err| panic!("getCommit {reference}: {err}"))
        .into_iter()
        .next()
}

/// GitHub Desktop's `GitError.message` (`lib/git/core.ts`) for a failed git
/// call: dugite's description of a recognised error, else git's output.
/// Corvene's text for it is `GitFailure::description` of the returned
/// error, else `GitFailure::output`; any other error is its `Display`.
pub fn git_error_message(err: &GitError) -> String {
    match err.failure() {
        Some(failure) => failure
            .description("Settings")
            .unwrap_or_else(|| failure.output.clone()),
        None => err.to_string(),
    }
}

/// GitHub Desktop's `getWorkingDirectoryDiff(repository, file)`
/// (`lib/git/diff.ts`): `corvene_git::working_directory_diff` with GitHub
/// Desktop's settings: whitespace shown, and the flags
/// `743-renamed-diff-against-head` and `749-binary-diff-as-text` off (their
/// `github-desktop` preset value).
///
/// # Panics
///
/// When git fails.
pub fn get_working_directory_diff(
    repository: &TestRepo,
    file: &WorkingDirectoryFileChange,
) -> Diff {
    corvene_git::working_directory_diff(git(), repository.path(), file, false, false, false)
        .unwrap_or_else(|err| panic!("getWorkingDirectoryDiff({}): {err}", file.path))
}

/// GitHub Desktop's `store.tip` after `new GitStore(repository, …)` and
/// `await store.loadStatus()` (`lib/stores/git-store.ts`): the tip
/// `corvene_git::open_repository` reads.
///
/// # Panics
///
/// When the repository cannot be read.
pub fn load_tip(repository: impl AsRef<Path>) -> Tip {
    let repository = repository.as_ref();
    corvene_git::open_repository(repository)
        .unwrap_or_else(|err| panic!("loadStatus in {}: {err}", repository.display()))
        .tip
}

/// GitHub Desktop's `createBareUpstream(t, source)`
/// (`unit/git/push-test.ts`): `git clone --bare <source> <temp>` (dugite's
/// `exec`, any exit code), to use as an upstream remote; bare repositories
/// accept pushes to any branch. The returned guard keeps the directory.
pub fn create_bare_upstream(source: &TestRepo) -> TempDir {
    let bare_path = create_temp_directory();
    exec(
        [
            OsStr::new("clone"),
            OsStr::new("--bare"),
            source.path().as_os_str(),
            bare_path.path().as_os_str(),
        ],
        source.path(),
    );
    bare_path
}
