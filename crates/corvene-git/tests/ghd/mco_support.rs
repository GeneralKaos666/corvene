//! Helpers shared by lane `mco`'s squash and reorder ports.

use std::path::PathBuf;

use corvene_git::RebaseResult;
use corvene_models::WorkingDirectoryFileChange;
use corvene_test_support::{TestRepo, create_temp_directory};
use tempfile::TempDir;

/// GitHub Desktop's `getTempFilePath(name)` (`lib/file-system.ts`): a path
/// named after `name` in a temporary location. The directory guard keeps it
/// until the test ends.
pub fn get_temp_file_path(name: &str) -> (TempDir, PathBuf) {
    let dir = create_temp_directory();
    let path = dir.path().join(name);
    (dir, path)
}

/// Stand-in for GitHub Desktop's `continueRebase(repository, files,
/// manualResolutions, { gitEditor })` (`lib/git/rebase.ts`): the tests pass
/// `gitEditor` (`GIT_EDITOR` for `rebase --continue`) to reword the stopped
/// commit. `corvene_git::continue_rebase` always runs with `GIT_EDITOR=:`
/// and has no editor parameter; once it has one, call it here (with no
/// manual resolutions, no commits for progress and `keep_messages: false`,
/// GitHub Desktop's value of flag `834-rebase-keeps-hash-messages`) and
/// remove the `#[ignore]`s.
pub fn continue_rebase(
    _repository: &TestRepo,
    _files: &[WorkingDirectoryFileChange],
    _git_editor: Option<&str>,
) -> RebaseResult {
    unimplemented!("corvene_git::continue_rebase has no git editor option (GHD opts.gitEditor)")
}
