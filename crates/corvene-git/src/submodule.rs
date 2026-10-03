//! Submodules - GHD `lib/git/submodule.ts` (`updateSubmodulesAfterOperation`,
//! `listSubmodules`, `resetSubmodulePaths`) and `models/submodule.ts`.
//!
//! `update_submodules_after_operation` takes no progress callback: Corvene's
//! checkout shows no progress (GHD reports "Updating submodules" in its
//! checkout progress). Discard (`commit::discard_changes`) tells submodules
//! apart by their status (`FileStatus::submodule`, the same information)
//! instead of `list_submodules`, which saves a git call per discard.

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::git_errors::KnownGitError;
use crate::process::GitCommand;
use crate::remote_ops::AskpassEnv;

/// GHD `SubmoduleEntry`: one line of `git submodule status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleEntry {
    /// The commit checked out in the submodule.
    pub sha: String,
    /// The submodule's path in the repository.
    pub path: String,
    /// `git describe` of that commit (`first-tag~2`, `heads/main`).
    pub describe: String,
}

/// GHD `AuthenticationErrors` (`lib/git/authentication.ts`).
const AUTHENTICATION_ERRORS: [KnownGitError; 4] = [
    KnownGitError::HTTPSAuthenticationFailed,
    KnownGitError::SSHAuthenticationFailed,
    KnownGitError::HTTPSRepositoryNotFound,
    KnownGitError::SSHRepositoryNotFound,
];

/// GHD `updateSubmodulesAfterOperation` (run after a branch checkout):
/// `git [-c protocol.file.allow=always] submodule update --init
/// --recursive`. `allow_file_protocol` lets submodules be cloned from a
/// local path; `askpass` answers credential prompts of submodules being
/// cloned. As in GHD, a submodule that cannot be cloned for want of
/// credentials (`AuthenticationErrors`) is not an error.
pub fn update_submodules_after_operation(
    git: Arc<GitBinary>,
    workdir: &Path,
    allow_file_protocol: bool,
    askpass: Option<&AskpassEnv>,
) -> Result<()> {
    let mut cmd = GitCommand::new(git)
        .current_dir(workdir)
        .expected_errors(AUTHENTICATION_ERRORS);
    if allow_file_protocol {
        cmd = cmd.args(["-c", "protocol.file.allow=always"]);
    }
    cmd = cmd.args(["submodule", "update", "--init", "--recursive"]);
    if let Some(askpass) = askpass {
        cmd = askpass.apply(cmd);
    }
    cmd.run()?;
    Ok(())
}

/// GHD `listSubmodules`: the top-level submodules (`git submodule status
/// --`). Without `.gitmodules`, `.git/modules` or `<common dir>/modules`
/// (a linked worktree) git is not asked and the list is empty; so is it
/// when git cannot read the submodules (exit 128).
pub fn list_submodules(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<SubmoduleEntry>> {
    if !workdir.join(".gitmodules").exists() && !workdir.join(".git").join("modules").exists() {
        let git_dir = crate::paths::git_dir(workdir);
        let common_dir = std::fs::read_to_string(git_dir.join("commondir"))
            .ok()
            .map(|content| content.trim_end_matches(['\r', '\n']).to_string())
            .filter(|p| !p.is_empty())
            .map(|p| git_dir.join(p));
        if !common_dir.is_some_and(|dir| dir.join("modules").exists()) {
            return Ok(Vec::new());
        }
    }
    let out = GitCommand::new(git)
        .args(["submodule", "status", "--"])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(parse_submodule_status(&out.stdout_string()?))
}

/// Parse `git submodule status` lines: a status character (` `, `-`, `+`,
/// `U`), the sha, the path and `(describe)` (GHD
/// `/^.([^ ]+) (.+) \((.+?)\)$/gm`). Lines without a describe are skipped.
pub fn parse_submodule_status(text: &str) -> Vec<SubmoduleEntry> {
    text.lines()
        .filter_map(|line| {
            let mut chars = line.chars();
            chars.next()?;
            let rest = chars.as_str();
            let (sha, rest) = rest.split_once(' ')?;
            if sha.is_empty() {
                return None;
            }
            let rest = rest.strip_suffix(')')?;
            // the describe is the shortest `(…)` at the end
            let open = rest.rfind(" (")?;
            let (path, describe) = (&rest[..open], &rest[open + 2..]);
            if path.is_empty() || describe.is_empty() {
                return None;
            }
            Some(SubmoduleEntry {
                sha: sha.to_string(),
                path: path.to_string(),
                describe: describe.to_string(),
            })
        })
        .collect()
}

/// GHD `resetSubmodulePaths`: `git submodule update --recursive --force --
/// <paths>`, back to the commit the index records with tracked changes
/// inside reverted (untracked files stay). Nothing for no paths.
pub fn reset_submodule_paths(git: Arc<GitBinary>, workdir: &Path, paths: &[&str]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    GitCommand::new(git)
        .args(["submodule", "update", "--recursive", "--force", "--"])
        .args(paths)
        .current_dir(workdir)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_submodule_status() {
        let text = " 1eaabe34fc6f486367a176207420378f587d3b48 git (v2.16.0-rc0)\n\
                    +c59617b65080863c4ca72c1f191fa1b423b92223 foo/sub module (heads/main)\n\
                    -0123456789012345678901234567890123456789 uninit\n";
        let entries = parse_submodule_status(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "git");
        assert_eq!(entries[0].describe, "v2.16.0-rc0");
        assert_eq!(entries[1].sha, "c59617b65080863c4ca72c1f191fa1b423b92223");
        assert_eq!(entries[1].path, "foo/sub module");
        assert_eq!(entries[1].describe, "heads/main");
    }
}
