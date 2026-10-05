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

/// Corvene (`1211-issuetracker-links`): an `[issuetracker "<name>"]`
/// section of a repository's `.issuetracker` file (the format GitLens and
/// Git Extensions read): `regex` finds references, `url` links them with
/// `$1`… for the regex's groups.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IssueTracker {
    pub name: String,
    pub regex: String,
    pub url: String,
}

/// The trackers of `workdir`'s `.issuetracker` with both a `regex` and a
/// `url` (`git config -f .issuetracker -z --get-regexp ^issuetracker\.`);
/// none when there is no such file.
pub fn issue_trackers(git: Arc<GitBinary>, workdir: &Path) -> Vec<IssueTracker> {
    if !workdir.join(".issuetracker").is_file() {
        return Vec::new();
    }
    GitCommand::new(git)
        .args([
            "config",
            "-f",
            ".issuetracker",
            "-z",
            "--get-regexp",
            r"^issuetracker\.",
        ])
        .current_dir(workdir)
        .allow_any_exit_code()
        .run()
        .ok()
        .map(|out| parse_issue_trackers(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

/// `git config -z --get-regexp` output (`<key>\n<value>\0` per entry) as
/// trackers, in the order they first appear.
fn parse_issue_trackers(output: &str) -> Vec<IssueTracker> {
    let mut trackers: Vec<IssueTracker> = Vec::new();
    for entry in output.split('\0') {
        let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
        let Some(rest) = key.strip_prefix("issuetracker.") else {
            continue;
        };
        let Some((name, variable)) = rest.rsplit_once('.') else {
            continue;
        };
        let at = match trackers.iter().position(|t| t.name == name) {
            Some(at) => at,
            None => {
                trackers.push(IssueTracker {
                    name: name.to_string(),
                    ..IssueTracker::default()
                });
                trackers.len() - 1
            }
        };
        match variable {
            "regex" => trackers[at].regex = value.to_string(),
            "url" => trackers[at].url = value.to_string(),
            _ => {}
        }
    }
    trackers.retain(|t| !t.regex.is_empty() && !t.url.is_empty());
    trackers
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

/// The branch `branch` was created from, as VS Code records it
/// (`branch.<branch>.vscode-merge-base`; Corvene writes it too, flag
/// `1202-update-from-parent-branch`).
pub fn branch_merge_base(git: Arc<GitBinary>, workdir: &Path, branch: &str) -> Option<String> {
    crate::config_value(git, workdir, &format!("branch.{branch}.vscode-merge-base"))
}

/// Records `parent` as the branch `branch` was created from
/// ([`branch_merge_base`]).
pub fn set_branch_merge_base(
    git: Arc<GitBinary>,
    workdir: &Path,
    branch: &str,
    parent: &str,
) -> Result<()> {
    set_local_config_value(
        git,
        workdir,
        &format!("branch.{branch}.vscode-merge-base"),
        parent,
    )
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

/// GHD `removeConfigValue`: `git config --local --unset-all <key>`; a
/// missing key (exit 5) is not an error (GHD's is).
/// `git config --global --unset-all <key>` (nothing to remove is fine).
pub fn remove_global_config_value(git: Arc<GitBinary>, key: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["config", "--global", "--unset-all", key])
        .allow_exit_code(5)
        .run()?;
    Ok(())
}

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
    fn reads_issue_trackers() {
        let git = Arc::new(find_git().expect("git"));
        let dir = tempfile::tempdir().unwrap();
        assert!(issue_trackers(git.clone(), dir.path()).is_empty());
        std::fs::write(
            dir.path().join(".issuetracker"),
            "[issuetracker \"Jira.Main\"]\n\tregex = \"(PROJ-\\\\d+)\"\n\
             \turl = \"https://jira.example/browse/$1\"\n\
             [issuetracker \"half\"]\n\tregex = x\n",
        )
        .unwrap();
        assert_eq!(
            issue_trackers(git, dir.path()),
            vec![IssueTracker {
                name: "Jira.Main".into(),
                regex: r"(PROJ-\d+)".into(),
                url: "https://jira.example/browse/$1".into(),
            }]
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
