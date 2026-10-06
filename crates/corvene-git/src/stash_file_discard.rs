//! Corvene (`1315-discard-stash-file`): take some files out of a stash
//! entry. GHD's stash viewer (`ui/stashing/stash-diff-viewer.tsx`) can only
//! restore or discard the whole stash.
//!
//! git has no command for this, so the entry's commits are rebuilt: each
//! of its trees (the working directory, the index and, for `stash -u`
//! entries, the untracked files) gets the files as they are in the commit
//! the stash was made on, through a temporary index, and `commit-tree`
//! makes new commits with the old ones' authors, dates and messages. The
//! new entry then takes the old one's place in `refs/stash`: the entries
//! above it are dropped and stored again around it, so the list keeps its
//! order (and the dates it shows, which are the commits' own). Nothing is
//! dropped before every new commit exists, and a failure while restacking
//! stores whatever is missing back.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use corvene_models::StashEntry;

use crate::branch_ops::{get_stashes, stash_entry_matching_sha};
use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

/// What [`discard_files_from_stash`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscardFromStash {
    /// The entry at `position` now has commit `new` instead of `old`.
    Rewritten {
        old: String,
        new: String,
        position: usize,
    },
    /// Those were the stash's only changes, so the entry at `position` was
    /// dropped.
    Dropped { old: String, position: usize },
    /// No stash entry has that commit (any more).
    Missing,
}

/// Removes the temporary index whatever happens.
struct TempIndex(PathBuf);

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Take `paths` (for a renamed file, both of its paths) out of the stash
/// entry whose commit is `stash_sha`.
pub fn discard_files_from_stash(
    git: Arc<GitBinary>,
    workdir: &Path,
    stash_sha: &str,
    paths: &[String],
) -> Result<DiscardFromStash> {
    let (entries, _) = get_stashes(git.clone(), workdir)?;
    let Some(position) = entries.iter().position(|e| e.sha == stash_sha) else {
        return Ok(DiscardFromStash::Missing);
    };
    let entry = &entries[position];
    let (base, index, untracked) = match entry.parents.as_slice() {
        [base, index] => (base, index, None),
        [base, index, untracked] => (base, index, Some(untracked)),
        _ => {
            return Err(GitError::Gix(
                "This stash entry does not look like one git made, so it cannot be changed.".into(),
            ));
        }
    };
    let zero = "0".repeat(stash_sha.len());
    let base_tree = rev_parse(git.clone(), workdir, &format!("{base}^{{tree}}"))?;
    let worktree_tree = tree_without(git.clone(), workdir, stash_sha, Some(base), paths, &zero)?;
    let index_tree = tree_without(git.clone(), workdir, index, Some(base), paths, &zero)?;
    let untracked_tree = match untracked {
        Some(u) => tree_without(git.clone(), workdir, u, None, paths, &zero)?,
        None => None,
    };
    let (Some(worktree_tree), Some(index_tree)) = (worktree_tree, index_tree) else {
        // a tree that was not empty before has every file the base has
        return Err(GitError::Gix("The stash's files could not be read.".into()));
    };
    if worktree_tree == base_tree && index_tree == base_tree && untracked_tree.is_none() {
        crate::branch_ops::drop_desktop_stash_entry(git, workdir, stash_sha)?;
        return Ok(DiscardFromStash::Dropped {
            old: stash_sha.to_string(),
            position,
        });
    }
    let new_index = commit_like(git.clone(), workdir, index, &index_tree, &[base])?;
    let new_untracked = match (untracked, &untracked_tree) {
        (Some(u), Some(tree)) => Some(commit_like(git.clone(), workdir, u, tree, &[])?),
        _ => None,
    };
    let mut parents = vec![base.as_str(), new_index.as_str()];
    parents.extend(new_untracked.as_deref());
    let new = commit_like(git.clone(), workdir, stash_sha, &worktree_tree, &parents)?;
    put_stash_entry(
        git,
        workdir,
        position,
        Some(stash_sha),
        &new,
        &entry.message,
    )?;
    Ok(DiscardFromStash::Rewritten {
        old: stash_sha.to_string(),
        new,
        position,
    })
}

/// Undo of [`discard_files_from_stash`]: put stash commit `sha` back with
/// `message` at `position`, in place of `replacing` when given (the entry
/// the discard made), else as a new entry there (a dropped stash).
pub fn put_back_stash_entry(
    git: Arc<GitBinary>,
    workdir: &Path,
    replacing: Option<&str>,
    position: usize,
    sha: &str,
    message: &str,
) -> Result<()> {
    let position = match replacing {
        Some(current) => {
            let (entries, _) = get_stashes(git.clone(), workdir)?;
            entries
                .iter()
                .position(|e| e.sha == current)
                .ok_or_else(|| {
                    GitError::Gix(
                        "The stash has changed since; it may have been restored or discarded."
                            .into(),
                    )
                })?
        }
        None => position,
    };
    put_stash_entry(git, workdir, position, replacing, sha, message)
}

/// `rev-parse --verify` of `rev`.
fn rev_parse(git: Arc<GitBinary>, workdir: &Path, rev: &str) -> Result<String> {
    Ok(GitCommand::new(git)
        .args(["rev-parse", "--verify", "-q", rev])
        .current_dir(workdir)
        .run()?
        .stdout_string()?
        .trim()
        .to_string())
}

/// The tree of `tree_ish` with `paths` as they are in `base` (gone when
/// `base` has none, or is `None`). `None` when nothing is left.
fn tree_without(
    git: Arc<GitBinary>,
    workdir: &Path,
    tree_ish: &str,
    base: Option<&str>,
    paths: &[String],
    zero: &str,
) -> Result<Option<String>> {
    let git_dir = crate::paths::git_dir(workdir);
    let index = TempIndex(git_dir.join(format!(
        "corvene-stash-file-{}-{}.index",
        std::process::id(),
        &tree_ish[..tree_ish.len().min(12)]
    )));
    let with_index = |cmd: GitCommand| cmd.env("GIT_INDEX_FILE", &index.0).current_dir(workdir);
    with_index(GitCommand::new(git.clone()).args(["read-tree", tree_ish])).run()?;
    // `--index-info` takes `mode SP sha TAB path` and `ls-tree` lines; mode
    // 0 removes the path
    let mut info: Vec<u8> = Vec::new();
    for path in paths {
        info.extend_from_slice(format!("0 {zero}\t{path}\0").as_bytes());
    }
    if let Some(base) = base {
        let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", base, "--"];
        args.extend(paths.iter().map(String::as_str));
        let listed = GitCommand::new(git.clone())
            .args(args)
            .current_dir(workdir)
            .run()?;
        for record in listed.stdout.split(|b| *b == 0) {
            let Some(tab) = record.iter().position(|b| *b == b'\t') else {
                continue;
            };
            let path = String::from_utf8_lossy(&record[tab + 1..]);
            // `ls-tree` paths are prefixes; only the files themselves count
            if paths.iter().any(|p| *p == path) {
                info.extend_from_slice(record);
                info.push(0);
            }
        }
    }
    with_index(
        GitCommand::new(git.clone())
            .args(["update-index", "-z", "--index-info"])
            .stdin(info),
    )
    .run()?;
    let left = with_index(GitCommand::new(git.clone()).args(["ls-files", "-z"])).run()?;
    if left.stdout.is_empty() && base.is_none() {
        return Ok(None);
    }
    let tree = with_index(GitCommand::new(git).args(["write-tree"]))
        .run()?
        .stdout_string()?;
    Ok(Some(tree.trim().to_string()))
}

/// A commit of `tree` over `parents` with commit `like`'s authors, dates
/// and message.
fn commit_like(
    git: Arc<GitBinary>,
    workdir: &Path,
    like: &str,
    tree: &str,
    parents: &[&str],
) -> Result<String> {
    let out = GitCommand::new(git.clone())
        .args([
            "log",
            "-1",
            "--no-show-signature",
            "--date=raw",
            "--format=%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%B",
            like,
        ])
        .current_dir(workdir)
        .run()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let fields: Vec<&str> = text.splitn(7, '\0').collect();
    let [an, ae, ad, cn, ce, cd, message] = fields.as_slice() else {
        return Err(GitError::Gix(format!(
            "Could not read stash commit {like}."
        )));
    };
    let mut args = vec!["commit-tree", "--no-gpg-sign", tree];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    Ok(GitCommand::new(git)
        .args(args)
        .env("GIT_AUTHOR_NAME", an)
        .env("GIT_AUTHOR_EMAIL", ae)
        .env("GIT_AUTHOR_DATE", ad)
        .env("GIT_COMMITTER_NAME", cn)
        .env("GIT_COMMITTER_EMAIL", ce)
        .env("GIT_COMMITTER_DATE", cd)
        .current_dir(workdir)
        .stdin(message.trim_end().as_bytes().to_vec())
        .run()?
        .stdout_string()?
        .trim()
        .to_string())
}

/// Store `sha` with `message` as `stash@{position}`, replacing the entry
/// there when it is `replacing`: the entries above are dropped, then stored
/// again on top of it.
fn put_stash_entry(
    git: Arc<GitBinary>,
    workdir: &Path,
    position: usize,
    replacing: Option<&str>,
    sha: &str,
    message: &str,
) -> Result<()> {
    let (entries, _) = get_stashes(git.clone(), workdir)?;
    let replaced = match replacing {
        Some(old) => {
            let Some(entry) = entries.get(position).filter(|e| e.sha == old) else {
                return Err(GitError::Gix(
                    "The stash list changed while the stash was being rewritten.".into(),
                ));
            };
            Some(entry.clone())
        }
        None => None,
    };
    let position = position.min(entries.len());
    let above: Vec<StashEntry> = entries[..position].to_vec();
    let removing = position + usize::from(replaced.is_some());
    let restack = || -> Result<()> {
        for _ in 0..removing {
            GitCommand::new(git.clone())
                .args(["stash", "drop", "-q", "stash@{0}"])
                .current_dir(workdir)
                .run()?;
        }
        crate::store_stash(git.clone(), workdir, sha, message)?;
        for entry in above.iter().rev() {
            crate::store_stash(git.clone(), workdir, &entry.sha, &entry.message)?;
        }
        Ok(())
    };
    let Err(err) = restack() else {
        return Ok(());
    };
    // whatever was taken off and is not back yet goes back on top
    for entry in replaced.iter().chain(above.iter().rev()) {
        if stash_entry_matching_sha(git.clone(), workdir, &entry.sha)
            .ok()
            .flatten()
            .is_none()
        {
            let _ = crate::store_stash(git.clone(), workdir, &entry.sha, &entry.message);
        }
    }
    Err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(path: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            // Git for Windows checks text out with CRLF unless told not to
            &["config", "core.autocrlf", "false"],
            &["config", "commit.gpgsign", "false"],
        ] {
            run(dir.path(), args);
        }
        std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "one\n").unwrap();
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "first"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    fn stash(dir: &Path, files: &[(&str, &str)], message: &str, untracked: bool) {
        for (name, text) in files {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let mut args = vec!["stash", "push", "-q", "-m", message];
        if untracked {
            args.push("-u");
        }
        run(dir, &args);
    }

    fn list(dir: &Path) -> Vec<String> {
        run(dir, &["stash", "list", "--format=%gs"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn files(dir: &Path, name: &str) -> Vec<String> {
        let mut files: Vec<String> = run(dir, &["stash", "show", "--name-only", "-u", name])
            .lines()
            .map(str::to_string)
            .collect();
        files.sort();
        files
    }

    #[test]
    fn discards_a_file_in_place() {
        let (dir, git) = repo();
        let p = dir.path();
        stash(p, &[("a.txt", "two\n"), ("b.txt", "two\n")], "older", false);
        stash(p, &[("a.txt", "three\n")], "newer", false);
        let (entries, _) = get_stashes(git.clone(), p).unwrap();
        let older = entries[1].clone();
        let done =
            discard_files_from_stash(git.clone(), p, &older.sha, &["a.txt".to_string()]).unwrap();
        let DiscardFromStash::Rewritten { new, position, .. } = done else {
            panic!("{done:?}");
        };
        assert_eq!(position, 1);
        assert_eq!(list(p), ["On main: newer", "On main: older"]);
        assert_eq!(files(p, "stash@{1}"), ["b.txt"]);
        assert_eq!(files(p, "stash@{0}"), ["a.txt"]);
        let (after, _) = get_stashes(git.clone(), p).unwrap();
        assert_eq!(after[1].sha, new);
        assert_eq!(after[1].date, older.date);
        assert_eq!(after[0].sha, entries[0].sha);
        // the rest still applies
        run(p, &["stash", "apply", "-q", "stash@{1}"]);
        assert_eq!(std::fs::read_to_string(p.join("b.txt")).unwrap(), "two\n");
        assert_eq!(std::fs::read_to_string(p.join("a.txt")).unwrap(), "one\n");
        // Undo puts the old commit back in its place
        put_back_stash_entry(git.clone(), p, Some(&new), 1, &older.sha, &older.message).unwrap();
        let (undone, _) = get_stashes(git, p).unwrap();
        assert_eq!(undone[1].sha, older.sha);
        assert_eq!(list(p), ["On main: newer", "On main: older"]);
    }

    #[test]
    fn untracked_and_staged_files_go_too() {
        let (dir, git) = repo();
        let p = dir.path();
        std::fs::write(p.join("staged.txt"), "new\n").unwrap();
        run(p, &["add", "staged.txt"]);
        stash(
            p,
            &[("a.txt", "two\n"), ("loose.txt", "x\n")],
            "mixed",
            true,
        );
        let sha = get_stashes(git.clone(), p).unwrap().0[0].sha.clone();
        let paths = ["staged.txt".to_string(), "loose.txt".to_string()];
        let done = discard_files_from_stash(git.clone(), p, &sha, &paths).unwrap();
        assert!(
            matches!(done, DiscardFromStash::Rewritten { .. }),
            "{done:?}"
        );
        assert_eq!(files(p, "stash@{0}"), ["a.txt"]);
        // the untracked parent went with its last file
        let parents = run(p, &["log", "-1", "--format=%P", "stash@{0}"]);
        assert_eq!(parents.split_whitespace().count(), 2);
        run(p, &["stash", "pop", "-q"]);
        assert!(!p.join("staged.txt").exists());
        assert!(!p.join("loose.txt").exists());
    }

    #[test]
    fn the_last_file_drops_the_entry() {
        let (dir, git) = repo();
        let p = dir.path();
        stash(p, &[("a.txt", "two\n")], "only", false);
        let entry = get_stashes(git.clone(), p).unwrap().0[0].clone();
        let done =
            discard_files_from_stash(git.clone(), p, &entry.sha, &["a.txt".to_string()]).unwrap();
        assert_eq!(
            done,
            DiscardFromStash::Dropped {
                old: entry.sha.clone(),
                position: 0
            }
        );
        assert!(list(p).is_empty());
        put_back_stash_entry(git, p, None, 0, &entry.sha, &entry.message).unwrap();
        assert_eq!(list(p), ["On main: only"]);
    }
}
