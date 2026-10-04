//! Port of `app/test/helpers/status.ts`.

use corvene_models::WorkingDirectoryStatus;

use crate::exec::git;
use crate::repositories::TestRepo;

/// GitHub Desktop's `getStatusOrThrow(repository)`: `getStatus` that fails
/// the test instead of returning `null`. Corvene's equivalent is
/// `corvene_git::get_status`, whose `Err` is GitHub Desktop's `null`
/// (git exited with 128) and whose `Ok` is a status with `exists: true`.
///
/// # Panics
///
/// When `git status` fails.
pub fn get_status_or_throw(repository: &TestRepo) -> WorkingDirectoryStatus {
    corvene_git::get_status(git(), repository.path())
        .unwrap_or_else(|err| panic!("git status returned null which was not expected: {err}"))
}
