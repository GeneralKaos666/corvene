//! Helpers shared by `corvene-git`'s ports of GitHub Desktop's squash and
//! reorder tests (`unit/git/squash-test.ts`, `unit/git/reorder-test.ts`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use corvene_git::RebaseResult;
use corvene_models::WorkingDirectoryFileChange;
use corvene_test_support::{TestRepo, create_temp_directory, git};
use tempfile::TempDir;

/// GitHub Desktop's `getTempFilePath(name)` (`lib/file-system.ts`): a path
/// named after `name` in a temporary location. The directory guard keeps it
/// until the test ends.
pub fn get_temp_file_path(name: &str) -> (TempDir, PathBuf) {
    let dir = create_temp_directory();
    let path = dir.path().join(name);
    (dir, path)
}

/// GitHub Desktop's `continueRebase(repository, files, manualResolutions,
/// { gitEditor })` (`lib/git/rebase.ts`) as the tests call it:
/// `corvene_git::continue_rebase` with no manual resolutions, no commits
/// for progress and `keep_messages: false` (GitHub Desktop's value of flag
/// `834-rebase-keeps-hash-messages`); `git_editor` is `GIT_EDITOR` for
/// `rebase --continue` (`None` is `:`). An `Err` is GitHub Desktop's
/// rejection and panics.
pub fn continue_rebase(
    repository: &TestRepo,
    files: &[WorkingDirectoryFileChange],
    git_editor: Option<&str>,
) -> RebaseResult {
    corvene_git::continue_rebase(
        git(),
        repository.path(),
        files,
        &BTreeMap::new(),
        &[],
        false,
        git_editor,
        |_| {},
    )
    .expect("continueRebase")
}
