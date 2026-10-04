//! The repository's `description` file (gitweb's), GHD
//! `app/src/lib/git/description.ts`. Publish Repository prefills its
//! Description field with it; Create a New Repository writes it.
//!
//! GHD joins `.git/description` to the repository path, so a repository
//! whose `.git` is a file (a submodule, a linked worktree, a
//! `--separate-git-dir` repository) always reads as having no description
//! there. Corvene reads and writes `description` in the directory the
//! gitfile points at ([`git_dir`], what `git rev-parse --git-path
//! description` names), where `git clone` and `git init` put it for a
//! submodule or a separate git dir. A linked worktree's own git dir has no
//! `description`, so it still reads `""` there, as in GHD.

use std::path::{Path, PathBuf};

use crate::paths::git_dir;

/// GHD `DefaultGitDescription`: what `git init` writes to `description`.
pub const DEFAULT_GIT_DESCRIPTION: &str =
    "Unnamed repository; edit this file 'description' to name the repository.\n";

fn description_path(workdir: &Path) -> PathBuf {
    git_dir(workdir).join("description")
}

/// GHD `getGitDescription`: the repository's description, or `""` when the
/// file is missing, unreadable or still holds git's
/// [`DEFAULT_GIT_DESCRIPTION`]. The text is returned as is (a trailing
/// newline included); bytes that are not UTF-8 become U+FFFD, as Node's
/// `readFile(path, 'utf8')` decodes them.
pub fn get_git_description(workdir: &Path) -> String {
    let Ok(bytes) = std::fs::read(description_path(workdir)) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    if text == DEFAULT_GIT_DESCRIPTION {
        String::new()
    } else {
        text.into_owned()
    }
}

/// GHD `writeGitDescription`: replace the repository's description with
/// `description`, written as is.
pub fn write_git_description(workdir: &Path, description: &str) -> std::io::Result<()> {
    std::fs::write(description_path(workdir), description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_gitfile() {
        let dir = tempfile::tempdir().unwrap();
        let workdir = dir.path().join("work");
        let real = dir.path().join("real.git");
        std::fs::create_dir_all(&workdir).unwrap();
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(workdir.join(".git"), "gitdir: ../real.git\n").unwrap();

        assert_eq!(get_git_description(&workdir), "");
        std::fs::write(real.join("description"), DEFAULT_GIT_DESCRIPTION).unwrap();
        assert_eq!(get_git_description(&workdir), "");

        write_git_description(&workdir, "the real thing").unwrap();
        assert_eq!(
            std::fs::read_to_string(real.join("description")).unwrap(),
            "the real thing"
        );
        assert_eq!(get_git_description(&workdir), "the real thing");
    }
}
