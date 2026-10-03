//! Git config reads and writes for the Settings dialogs - GHD `lib/git/config.ts`
//! (`getConfigValue`, `getGlobalConfigValue`, `setConfigValue`,
//! `setGlobalConfigValue`, `removeConfigValue`, `setDefaultBranch`).

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

fn value_of(cmd: GitCommand) -> Option<String> {
    cmd.allow_exit_code(1)
        .run()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `git config --local --get <key>` inside `workdir`.
pub fn local_config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Option<String> {
    value_of(
        GitCommand::new(git)
            .args(["config", "--local", "--get", key])
            .current_dir(workdir),
    )
}

/// `git config --global --get <key>`.
pub fn global_config_value(git: Arc<GitBinary>, key: &str) -> Option<String> {
    value_of(GitCommand::new(git).args(["config", "--global", "--get", key]))
}

/// `git config --global --get-all <key>`: every value of a multi-valued key.
pub fn global_config_values(git: Arc<GitBinary>, key: &str) -> Vec<String> {
    GitCommand::new(git)
        .args(["config", "--global", "--get-all", key])
        .allow_exit_code(1)
        .run()
        .ok()
        .and_then(|out| out.stdout_string().ok())
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// `git config --local <key> <value>`.
pub fn set_local_config_value(
    git: Arc<GitBinary>,
    workdir: &Path,
    key: &str,
    value: &str,
) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", key, value])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `git config --global <key> <value>`.
pub fn set_global_config_value(git: Arc<GitBinary>, key: &str, value: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--global", key, value])
        .run()?;
    Ok(())
}

/// GHD `addSafeDirectory`: `git config --global --add safe.directory <path>`
/// ("Trust Repository" for a repository git considers unsafe).
pub fn add_safe_directory(git: Arc<GitBinary>, path: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--global", "--add", "safe.directory"])
        .arg(path.to_string_lossy().as_ref())
        .run()?;
    Ok(())
}

/// Flag `explain-trust-failure`: whether git still refuses `path` as an
/// unsafe repository (checked with `rev-parse --git-dir`). `Some` carries
/// the `safe.directory` value git suggests in its refusal, when it names one.
pub fn still_unsafe(git: Arc<GitBinary>, path: &Path) -> Option<Option<String>> {
    match GitCommand::new(git)
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .run()
    {
        Err(crate::error::GitError::Failed { stderr, .. })
            if crate::error::dubious_ownership_path(&stderr).is_some() =>
        {
            Some(suggested_safe_directory(&stderr))
        }
        _ => None,
    }
}

/// The value in git's "git config --global --add safe.directory <value>"
/// advice, unquoted.
fn suggested_safe_directory(stderr: &str) -> Option<String> {
    const MARKER: &str = "--add safe.directory ";
    let line = stderr.lines().find(|l| l.contains(MARKER))?;
    let value = line[line.find(MARKER)? + MARKER.len()..].trim();
    let value = value
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .unwrap_or(value);
    (!value.is_empty()).then(|| value.to_string())
}

/// `git config --local --unset <key>`; a missing key (exit 5) is not an error.
pub fn remove_local_config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", "--unset", key])
        .current_dir(workdir)
        .allow_exit_code(5)
        .run()?;
    Ok(())
}

/// GHD `setDefaultBranch`: `init.defaultBranch` in the global config.
pub fn set_default_branch(git: Arc<GitBinary>, name: &str) -> Result<()> {
    set_global_config_value(git, "init.defaultBranch", name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::find_git;

    #[test]
    fn reads_the_suggested_safe_directory() {
        let stderr = "fatal: detected dubious ownership in repository at '//server/share/repo'\n\
                      To add an exception for this directory, call:\n\n\
                      \tgit config --global --add safe.directory '%(prefix)///server/share/repo'\n";
        assert_eq!(
            suggested_safe_directory(stderr).as_deref(),
            Some("%(prefix)///server/share/repo")
        );
        assert_eq!(
            suggested_safe_directory("fatal: not a git repository"),
            None
        );
    }

    #[test]
    fn local_values_round_trip() {
        let git = Arc::new(find_git().expect("git"));
        let dir = tempfile::tempdir().unwrap();
        GitCommand::new(git.clone())
            .args(["init", "-q"])
            .current_dir(dir.path())
            .run()
            .unwrap();
        assert_eq!(
            local_config_value(git.clone(), dir.path(), "user.name"),
            None
        );
        set_local_config_value(git.clone(), dir.path(), "user.name", "Local Ada").unwrap();
        assert_eq!(
            local_config_value(git.clone(), dir.path(), "user.name").as_deref(),
            Some("Local Ada")
        );
        remove_local_config_value(git.clone(), dir.path(), "user.name").unwrap();
        remove_local_config_value(git.clone(), dir.path(), "user.name").unwrap();
        assert_eq!(local_config_value(git, dir.path(), "user.name"), None);
    }
}
