//! Port of `app/test/helpers/status.ts`, and the check that Corvene's
//! in-process readers (flags `906-in-process-status` and
//! `907-in-process-commit-files`) answer every GitHub Desktop test exactly
//! as git does.

use std::path::Path;

use corvene_models::{ChangesetData, WorkingDirectoryStatus};

use crate::exec::git;
use crate::repositories::TestRepo;

/// GitHub Desktop's `getStatusOrThrow(repository)`: `getStatus` that fails
/// the test instead of returning `null`. Corvene's equivalent is
/// `corvene_git::get_status`, whose `Err` is GitHub Desktop's `null`
/// (git exited with 128) and whose `Ok` is a status with `exists: true`.
///
/// The status is read both ways Corvene can read it: by `git status` and by
/// gitoxide in-process (flag `906-in-process-status`,
/// [`check_in_process_status`]), and the two must be identical, so every
/// expectation a test has of the result holds for both.
///
/// # Panics
///
/// When `git status` fails, or the in-process status differs from it.
pub fn get_status_or_throw(repository: &TestRepo) -> WorkingDirectoryStatus {
    let status = corvene_git::get_status(git(), repository.path())
        .unwrap_or_else(|err| panic!("git status returned null which was not expected: {err}"));
    check_in_process_status(repository.path(), &status);
    status
}

/// Fails the test unless the in-process status of `path`
/// ([`corvene_git::get_status_in_process`], flag `906-in-process-status`)
/// is `status`, the one `git status` gave. Where the in-process reader
/// leaves the repository to git (`None`) there is nothing to compare: the
/// app runs git there too. `CORVENE_GHD_IN_PROCESS_REPORT=1` prints those
/// cases (with `--nocapture`).
///
/// # Panics
///
/// When the two differ.
pub fn check_in_process_status(path: &Path, status: &WorkingDirectoryStatus) {
    match corvene_git::get_status_in_process(git(), path, Default::default()) {
        Some(in_process) => assert_eq!(
            &in_process,
            status,
            "the in-process status (left) differs from git status (right) in {}",
            path.display()
        ),
        None => report_fallback("status", path),
    }
}

/// GitHub Desktop's `getChangedFiles(repository, sha)` (`lib/git/log.ts`):
/// `corvene_git::get_changed_files` with git (`git log -C -M -m -1
/// --first-parent --raw --numstat -z`), checked against the in-process
/// reader like [`get_status_or_throw`] (flag
/// `907-in-process-commit-files`, [`check_in_process_changed_files`]).
///
/// # Errors
///
/// When git fails (GitHub Desktop's promise rejects).
///
/// # Panics
///
/// When the in-process files differ from git's.
pub fn get_changed_files(
    repository: &TestRepo,
    sha: &str,
) -> Result<ChangesetData, corvene_git::GitError> {
    let data = corvene_git::get_changed_files(git(), repository.path(), sha, false)?;
    check_in_process_changed_files(repository.path(), sha, sha, &data);
    Ok(data)
}

/// Fails the test unless the in-process changed files from `oldest`'s
/// parent to `newest` ([`corvene_git::get_changed_files_in_process`]) are
/// `data`, git's. `None` (left to git) is reported as in
/// [`check_in_process_status`].
///
/// # Panics
///
/// When the two differ.
pub fn check_in_process_changed_files(
    path: &Path,
    oldest: &str,
    newest: &str,
    data: &ChangesetData,
) {
    match corvene_git::get_changed_files_in_process(path, oldest, newest) {
        Some(in_process) => assert_eq!(
            &in_process,
            data,
            "the in-process changed files (left) differ from git's (right) for \
             {oldest}..{newest} in {}",
            path.display()
        ),
        None => report_fallback(&format!("changed files of {oldest}..{newest}"), path),
    }
}

fn report_fallback(what: &str, path: &Path) {
    if std::env::var_os("CORVENE_GHD_IN_PROCESS_REPORT").is_some() {
        eprintln!(
            "in-process {what} left to git: {} in {}",
            std::thread::current().name().unwrap_or("?"),
            path.display()
        );
    }
}
