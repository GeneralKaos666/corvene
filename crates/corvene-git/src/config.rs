//! Git config reads and writes for the Settings dialogs - GHD `lib/git/config.ts`
//! (`getConfigValue`, `getGlobalConfigValue`, `getBooleanConfigValue`,
//! `getGlobalBooleanConfigValue`, `getGlobalConfigPath`, `setConfigValue`,
//! `setGlobalConfigValue`, `addSafeDirectory`, `removeConfigValue`,
//! `setDefaultBranch`).

use std::path::{Path, PathBuf};
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

/// GHD `getConfigValueInPath(.., 'bool')`: `git config -z [--global |
/// --local] --type bool <key>`, `None` when unset (exit 1), else whether
/// git's canonical value differs from `false` (`off`, `no` and `0` are
/// false; `on`, `yes` and `1` true).
fn boolean_value_of(cmd: GitCommand) -> Option<bool> {
    let out = cmd.allow_exit_code(1).run().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.split('\0').next().unwrap_or_default() != "false")
}

/// GHD `getBooleanConfigValue`: `key` as git reads it in `workdir` (every
/// scope, or the repository's own with `only_local`), canonicalised as a
/// boolean.
pub fn boolean_config_value(
    git: Arc<GitBinary>,
    workdir: &Path,
    key: &str,
    only_local: bool,
) -> Option<bool> {
    let mut cmd = GitCommand::new(git)
        .args(["config", "-z"])
        .current_dir(workdir);
    if only_local {
        cmd = cmd.arg("--local");
    }
    boolean_value_of(cmd.args(["--type", "bool", key]))
}

/// GHD `getGlobalBooleanConfigValue`: the global `key` canonicalised as a
/// boolean.
pub fn global_boolean_config_value(git: Arc<GitBinary>, key: &str) -> Option<bool> {
    boolean_value_of(GitCommand::new(git).args(["config", "-z", "--global", "--type", "bool", key]))
}

/// GHD `getGlobalConfigPath`: the global config file as git sees it
/// (`GIT_CONFIG_GLOBAL`, `~/.gitconfig` or the XDG file), the path `git
/// config --edit --global` hands its editor (`GIT_EDITOR='printf %s'`),
/// normalised. git creates the file when it does not exist yet.
pub fn global_config_path(git: Arc<GitBinary>) -> Result<PathBuf> {
    let out = GitCommand::new(git)
        .args(["config", "--edit", "--global"])
        .env("GIT_EDITOR", "printf %s")
        .run()?;
    let path = PathBuf::from(out.stdout_string()?);
    // `path.normalize`: drop `.` components and resolve `..` lexically
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
                if matches!(
                    normalized.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) =>
            {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

/// GHD `setConfigValue`: `git config --replace-all <key> <value>` in
/// `workdir`'s own config, so a key with several values ends up with one.
pub fn set_local_config_value(
    git: Arc<GitBinary>,
    workdir: &Path,
    key: &str,
    value: &str,
) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", "--replace-all", key, value])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// GHD `setGlobalConfigValue`: `git config --global --replace-all <key>
/// <value>`.
pub fn set_global_config_value(git: Arc<GitBinary>, key: &str, value: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--global", "--replace-all", key, value])
        .run()?;
    Ok(())
}

/// GHD `addSafeDirectory` (`addGlobalConfigValueIfMissing`): `git config
/// --global --add safe.directory <path>` unless the value is already there
/// ("Trust Repository" for a repository git considers unsafe). On Windows a
/// UNC path (`//server/share`) gets git's `%(prefix)/` in front.
pub fn add_safe_directory(git: Arc<GitBinary>, path: &Path) -> Result<()> {
    let mut value = path.to_string_lossy().into_owned();
    if cfg!(windows) && value.starts_with('/') {
        value = format!("%(prefix)/{value}");
    }
    let out = GitCommand::new(git.clone())
        .args(["config", "--global", "-z", "--get-all", "safe.directory"])
        .allow_exit_code(1)
        .run()?;
    let present = out.status.success()
        && String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .any(|v| v == value);
    if !present {
        GitCommand::new(git)
            .args(["config", "--global", "--add", "safe.directory"])
            .arg(&value)
            .run()?;
    }
    Ok(())
}

/// GHD `removeConfigValue`: `git config --local --unset-all <key>`; a
/// missing key (exit 5) is not an error (GHD's is).
pub fn remove_local_config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--local", "--unset-all", key])
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
