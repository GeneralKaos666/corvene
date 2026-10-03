//! Repository path helpers.

use std::path::{Path, PathBuf};

/// Resolve `<workdir>/.git`, following a gitfile (`gitdir: …`) as used by
/// worktrees and submodules (GHD `resolvedGitDir`).
pub fn git_dir(workdir: &Path) -> PathBuf {
    let dot_git = workdir.join(".git");
    if dot_git.is_file()
        && let Ok(text) = std::fs::read_to_string(&dot_git)
        && let Some(rest) = text.trim().strip_prefix("gitdir:")
    {
        let target = Path::new(rest.trim());
        return if target.is_absolute() {
            target.to_path_buf()
        } else {
            workdir.join(target)
        };
    }
    dot_git
}

/// The repository's common directory: for a linked worktree the main
/// `.git` named by `<gitdir>/commondir`, else [`git_dir`] itself.
pub fn common_dir(workdir: &Path) -> PathBuf {
    let dir = git_dir(workdir);
    match std::fs::read_to_string(dir.join("commondir")) {
        Ok(text) if !text.trim().is_empty() => {
            let target = Path::new(text.trim());
            if target.is_absolute() {
                target.to_path_buf()
            } else {
                dir.join(target)
            }
        }
        _ => dir,
    }
}
