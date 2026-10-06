//! Corvene `1113-lfs-locks`: Git LFS file locks (`git lfs locks`, `lock`,
//! `unlock`). GHD has none; it only warns when a push fails on a server
//! without the locking API (desktop#2812).
//!
//! `git lfs locks --json` prints `[]` and exits 0 when the server answers
//! 404 (no locking API), so an empty list is checked once more without
//! `--json`, which fails then ("Error while retrieving locks: Not Found").

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;
use crate::remote_ops::AskpassEnv;

/// One lock on the LFS server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LfsLock {
    pub id: String,
    /// Repository-relative, `/`-separated.
    pub path: String,
    /// The holder's name, as the server reports it.
    pub owner: String,
    pub locked_at: Option<String>,
    /// The lock is the user's (`--verify`); `None` when the server cannot
    /// say (no `locks/verify` endpoint).
    pub ours: Option<bool>,
}

/// What the LFS server says about locks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LfsLocks {
    /// The server has no locking API (or there is no LFS server).
    Unsupported,
    Locks(Vec<LfsLock>),
}

#[derive(Deserialize)]
struct JsonOwner {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct JsonLock {
    id: String,
    path: String,
    #[serde(default)]
    owner: Option<JsonOwner>,
    #[serde(default)]
    locked_at: Option<String>,
}

#[derive(Deserialize)]
struct JsonVerify {
    #[serde(default)]
    ours: Vec<JsonLock>,
    #[serde(default)]
    theirs: Vec<JsonLock>,
}

impl JsonLock {
    fn into_lock(self, ours: Option<bool>) -> LfsLock {
        LfsLock {
            id: self.id,
            path: self.path,
            owner: self.owner.map(|o| o.name).unwrap_or_default(),
            locked_at: self.locked_at,
            ours,
        }
    }
}

fn lfs_command(git: Arc<GitBinary>, workdir: &Path, askpass: Option<&AskpassEnv>) -> GitCommand {
    let cmd = GitCommand::new(git).current_dir(workdir).arg("lfs");
    match askpass {
        Some(askpass) => askpass.apply(cmd),
        None => cmd,
    }
}

/// Parse `git lfs locks --verify --json`.
pub fn parse_verified_locks(text: &str) -> Option<Vec<LfsLock>> {
    let verify: JsonVerify = serde_json::from_str(text.trim()).ok()?;
    Some(
        verify
            .ours
            .into_iter()
            .map(|l| l.into_lock(Some(true)))
            .chain(verify.theirs.into_iter().map(|l| l.into_lock(Some(false))))
            .collect(),
    )
}

/// Parse `git lfs locks --json` (no owner check).
pub fn parse_locks(text: &str) -> Option<Vec<LfsLock>> {
    let locks: Vec<JsonLock> = serde_json::from_str(text.trim()).ok()?;
    Some(locks.into_iter().map(|l| l.into_lock(None)).collect())
}

/// Every lock on the repository's LFS server, sorted by path: `git lfs
/// locks --verify --json`, which tells the user's locks apart; then, when
/// that has none (a server without `locks/verify` answers nothing), the
/// plain list; and when that is empty too, whether the server has a
/// locking API at all.
pub fn lfs_locks(
    git: Arc<GitBinary>,
    workdir: &Path,
    askpass: Option<&AskpassEnv>,
) -> Result<LfsLocks> {
    let mut locks = lfs_command(git.clone(), workdir, askpass)
        .args(["locks", "--verify", "--json"])
        .allow_any_exit_code()
        .run()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| parse_verified_locks(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    if locks.is_empty() {
        let out = lfs_command(git.clone(), workdir, askpass)
            .args(["locks", "--json"])
            .allow_any_exit_code()
            .run()?;
        if out.status.success() {
            locks = parse_locks(&String::from_utf8_lossy(&out.stdout)).unwrap_or_default();
        }
    }
    if locks.is_empty() {
        let out = lfs_command(git, workdir, askpass)
            .args(["locks", "--limit", "1"])
            .allow_any_exit_code()
            .run()?;
        if !out.status.success() {
            return Ok(LfsLocks::Unsupported);
        }
    }
    locks.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(LfsLocks::Locks(locks))
}

/// `git lfs lock <path>` for each path, stopping at the first failure
/// ("Locking a.psd failed: already created lock").
pub fn lfs_lock(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
    askpass: Option<&AskpassEnv>,
) -> Result<()> {
    for path in paths {
        lfs_command(git.clone(), workdir, askpass)
            .args(["lock", "--json", "--"])
            .arg(path)
            .run()?;
    }
    Ok(())
}

/// `git lfs unlock [--force] --id=<id>` for each lock (by id, so deleted
/// files unlock too). `--json` reports a refused unlock in its output with
/// exit code 2; its `reason` becomes the error.
pub fn lfs_unlock(
    git: Arc<GitBinary>,
    workdir: &Path,
    ids: &[String],
    force: bool,
    askpass: Option<&AskpassEnv>,
) -> Result<()> {
    for id in ids {
        let mut cmd = lfs_command(git.clone(), workdir, askpass).args(["unlock", "--json"]);
        if force {
            cmd = cmd.arg("--force");
        }
        let out = cmd.arg(format!("--id={id}")).allow_any_exit_code().run()?;
        if !out.status.success() {
            #[derive(Deserialize)]
            struct Unlocked {
                #[serde(default)]
                reason: Option<String>,
            }
            let reason = serde_json::from_slice::<Vec<Unlocked>>(&out.stdout)
                .ok()
                .and_then(|v| v.into_iter().find_map(|u| u.reason))
                .unwrap_or_else(|| out.stderr.trim().to_string());
            return Err(GitError::Failed {
                args: "lfs unlock".to_string(),
                code: out.status.code(),
                stderr: reason,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verified_and_plain_lists() {
        let verified = r#"{"ours":[{"id":"2","path":"art/logo.psd","owner":{"name":"Me"},"locked_at":"2024-03-04T10:00:00Z"}],"theirs":[{"id":"1","path":"art/hero.psd","owner":{"name":"Mona Lisa"},"locked_at":"2024-03-04T10:00:00Z"}]}"#;
        let locks = parse_verified_locks(verified).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].path, "art/logo.psd");
        assert_eq!(locks[0].ours, Some(true));
        assert_eq!(locks[1].owner, "Mona Lisa");
        assert_eq!(locks[1].ours, Some(false));
        let plain = r#"[{"id":"1","path":"a b.psd","owner":{"name":"X"}}]"#;
        let locks = parse_locks(plain).unwrap();
        assert_eq!(locks[0].path, "a b.psd");
        assert_eq!(locks[0].ours, None);
        assert_eq!(locks[0].locked_at, None);
        assert_eq!(parse_locks("[]").unwrap(), Vec::new());
        assert!(parse_locks("Error").is_none());
    }
}
