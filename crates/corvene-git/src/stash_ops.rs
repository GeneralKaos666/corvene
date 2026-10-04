//! Corvene's stash steps beyond GHD's `lib/git/stash.ts` (ported in
//! [`crate::branch_ops`]).
//!
//! Deviation (`774-stash-conflict-flow`): a restore whose changes conflict
//! keeps the stash entry git kept ([`StashPopOptions::keep_on_conflict`]);
//! GHD's `popStashEntry` takes `git stash pop`'s exit code 1 with nothing on
//! stderr for success and drops the entry. The conflicted files are marked
//! resolved with [`mark_conflicts_resolved`] (the index only).
//!
//! Deviation (`775-stash-restore-unstages-new-files`): Desktop stashes add
//! untracked files to the index first (`createDesktopStashEntry`), so a pop
//! brings them back staged as new files, and a branch whose `.gitignore`
//! ignores them still lists them; with
//! [`StashPopOptions::unstage_new_files`] a restore unstages the files the
//! stash added ([`stash_new_files`]) and they are untracked again.

use std::path::Path;
use std::sync::Arc;

use corvene_models::StashEntry;

use crate::branch_ops::{drop_desktop_stash_entry, stash_entry_matching_sha};
use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::git_errors::{KnownGitError, known_git_error};
use crate::process::GitCommand;

/// How [`pop_stash_entry_with`] goes beyond GHD's `popStashEntry`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StashPopOptions {
    /// `774-stash-conflict-flow`: a pop that leaves conflicted files keeps
    /// the entry (GHD drops it).
    pub keep_on_conflict: bool,
    /// `775-stash-restore-unstages-new-files`: after a restore, the files
    /// the stash added are unstaged (GHD leaves them staged).
    pub unstage_new_files: bool,
}

/// What [`pop_stash_entry_with`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashPop {
    /// No stash entry has that commit (any more).
    Missing,
    /// The changes are back and the entry is gone.
    Restored,
    /// The changes are back with conflicts and the entry is still there
    /// (always with [`StashPopOptions::keep_on_conflict`]; without it only
    /// when git named the conflicts, as GHD's `MergeConflicts`).
    Conflicted,
}

/// `popStashEntry` with [`StashPopOptions`]: `git stash pop --quiet <name>`
/// of the entry whose commit is `stash_sha`. Without options it is GHD's
/// exactly: exit code 1 with nothing on stderr counts as applied and the
/// entry is dropped.
pub fn pop_stash_entry_with(
    git: Arc<GitBinary>,
    workdir: &Path,
    stash_sha: &str,
    options: StashPopOptions,
) -> Result<StashPop> {
    let Some(entry) = stash_entry_matching_sha(git.clone(), workdir, stash_sha)? else {
        return Ok(StashPop::Missing);
    };
    let new_files = if options.unstage_new_files {
        stash_new_files(git.clone(), workdir, &entry).unwrap_or_default()
    } else {
        Vec::new()
    };
    // conflicts already there are not this pop's (git refuses to apply)
    let unmerged_before =
        options.keep_on_conflict && !unmerged_paths(git.clone(), workdir)?.is_empty();
    let out = GitCommand::new(git.clone())
        .args(["stash", "pop", "--quiet", &entry.name])
        .current_dir(workdir)
        .allow_any_exit_code()
        .run()?;
    if out.status.success() {
        unstage_new_files(git, workdir, &new_files);
        return Ok(StashPop::Restored);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let known = known_git_error(&out.stderr).or_else(|| known_git_error(&stdout));
    if known == Some(KnownGitError::MergeConflicts) {
        return Ok(StashPop::Conflicted);
    }
    if out.status.code() == Some(1)
        && options.keep_on_conflict
        && !unmerged_before
        && !unmerged_paths(git.clone(), workdir)?.is_empty()
    {
        return Ok(StashPop::Conflicted);
    }
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        drop_desktop_stash_entry(git.clone(), workdir, stash_sha)?;
        unstage_new_files(git, workdir, &new_files);
        return Ok(StashPop::Restored);
    }
    Err(GitError::Failed {
        args: format!("stash pop --quiet {}", entry.name),
        code: out.status.code(),
        stderr: out.stderr.trim().to_string(),
    })
}

/// The files stash `entry` added to the index: those its index commit
/// (second parent) has and its base (first parent) does not. For a Desktop
/// stash that is every file that was untracked, plus new files that were
/// staged.
pub fn stash_new_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    entry: &StashEntry,
) -> Result<Vec<String>> {
    let [base, index, ..] = entry.parents.as_slice() else {
        return Ok(Vec::new());
    };
    let out = GitCommand::new(git)
        .args([
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=A",
            base,
            index,
        ])
        .current_dir(workdir)
        .run()?;
    Ok(split_nul(&out.stdout))
}

/// After a restore: unstage those of `added` that are staged as new files
/// now (index only). A failure leaves them staged; the restore stands.
fn unstage_new_files(git: Arc<GitBinary>, workdir: &Path, added: &[String]) {
    if added.is_empty() {
        return;
    }
    let staged = GitCommand::new(git.clone())
        .args([
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=A",
        ])
        .current_dir(workdir)
        .run()
        .map(|out| split_nul(&out.stdout));
    let result = staged.and_then(|staged| {
        let paths: Vec<String> = added
            .iter()
            .filter(|p| staged.contains(p))
            .cloned()
            .collect();
        mark_conflicts_resolved(git, workdir, &paths)
    });
    if let Err(err) = result {
        tracing::warn!(%err, "could not unstage the files the stash added");
    }
}

fn split_nul(stdout: &[u8]) -> Vec<String> {
    stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// The paths with unmerged index entries (`ls-files -u`), each once.
pub fn unmerged_paths(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<String>> {
    let out = GitCommand::new(git)
        .args(["ls-files", "-u", "-z"])
        .current_dir(workdir)
        .run()?;
    Ok(parse_unmerged_paths(&out.stdout))
}

/// `ls-files -u -z` records: `<mode> <sha> <stage>\t<path>` NUL.
pub fn parse_unmerged_paths(stdout: &[u8]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for record in stdout.split(|b| *b == 0) {
        let record = String::from_utf8_lossy(record);
        if let Some((_, path)) = record.split_once('\t')
            && paths.last().map(String::as_str) != Some(path)
        {
            paths.push(path.to_string());
        }
    }
    paths
}

/// `774-stash-conflict-flow` › Mark as Resolved: `git reset -q -- <paths>`.
/// The unmerged index entries go back to `HEAD`'s; the working files, as
/// resolved, stay as they are (as changes of their own).
pub fn mark_conflicts_resolved(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    GitCommand::new(git)
        .args([
            "reset",
            "-q",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ])
        .env("GIT_LITERAL_PATHSPECS", "1")
        .current_dir(workdir)
        .stdin(nul_separated(paths))
        .run()?;
    Ok(())
}

/// The entry of `refs/stash` whose commit is `sha`, if it is still there.
pub fn stash_entry(git: Arc<GitBinary>, workdir: &Path, sha: &str) -> Result<Option<StashEntry>> {
    stash_entry_matching_sha(git, workdir, sha)
}

fn nul_separated(paths: &[String]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for path in paths {
        out.extend_from_slice(path.as_bytes());
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "commit.gpgsign", "false"],
            &["config", "user.name", "T"],
            &["config", "user.email", "t@example.com"],
        ] {
            run(dir.path(), args);
        }
        std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "first"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    fn run(path: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    }

    /// A stash of `a.txt` = "two" made on "one", then "three" committed.
    fn conflicting_stash() -> (tempfile::TempDir, Arc<GitBinary>, String) {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "two\n").unwrap();
        assert!(crate::create_desktop_stash(git.clone(), path, "main", false).unwrap());
        std::fs::write(path.join("a.txt"), "three\n").unwrap();
        run(path, &["commit", "-q", "-am", "three"]);
        let sha = crate::get_stashes(git.clone(), path).unwrap().0[0]
            .sha
            .clone();
        (dir, git, sha)
    }

    #[test]
    fn parses_unmerged_paths_once_each() {
        let out = b"100644 aaa 1\ta.txt\x00100644 bbb 2\ta.txt\x00100644 ccc 3\ta.txt\x00100644 ddd 2\tb c.txt\x00";
        assert_eq!(parse_unmerged_paths(out), ["a.txt", "b c.txt"]);
        assert!(parse_unmerged_paths(b"").is_empty());
    }

    #[test]
    fn conflicting_pop_keeps_the_entry_when_asked() {
        let (dir, git, sha) = conflicting_stash();
        let path = dir.path();
        let options = StashPopOptions {
            keep_on_conflict: true,
            ..Default::default()
        };
        assert_eq!(
            pop_stash_entry_with(git.clone(), path, &sha, options).unwrap(),
            StashPop::Conflicted
        );
        assert_eq!(unmerged_paths(git.clone(), path).unwrap(), ["a.txt"]);
        assert!(stash_entry(git.clone(), path, &sha).unwrap().is_some());
        // resolved in an editor, then marked: index only, the file stays
        std::fs::write(path.join("a.txt"), "two and three\n").unwrap();
        mark_conflicts_resolved(git.clone(), path, &["a.txt".to_string()]).unwrap();
        assert!(unmerged_paths(git.clone(), path).unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "two and three\n"
        );
        assert!(stash_entry(git, path, &sha).unwrap().is_some());
    }

    #[test]
    fn conflicting_pop_drops_the_entry_like_ghd_by_default() {
        let (dir, git, sha) = conflicting_stash();
        let path = dir.path();
        assert_eq!(
            pop_stash_entry_with(git.clone(), path, &sha, StashPopOptions::default()).unwrap(),
            StashPop::Restored
        );
        assert!(stash_entry(git, path, &sha).unwrap().is_none());
    }

    #[test]
    fn restore_can_leave_stashed_new_files_untracked() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "two\n").unwrap();
        std::fs::write(path.join("new.txt"), "n\n").unwrap();
        assert!(crate::create_desktop_stash(git.clone(), path, "main", false).unwrap());
        let entry = crate::get_stashes(git.clone(), path).unwrap().0.remove(0);
        assert_eq!(
            stash_new_files(git.clone(), path, &entry).unwrap(),
            ["new.txt"]
        );
        let options = StashPopOptions {
            unstage_new_files: true,
            ..Default::default()
        };
        assert_eq!(
            pop_stash_entry_with(git.clone(), path, &entry.sha, options).unwrap(),
            StashPop::Restored
        );
        let status = crate::get_status(git.clone(), path).unwrap();
        let new = status.files.iter().find(|f| f.path == "new.txt").unwrap();
        assert_eq!(new.status.kind, corvene_models::FileStatusKind::Untracked);
        let modified = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!(
            modified.status.kind,
            corvene_models::FileStatusKind::Modified
        );
        assert!(stash_entry(git, path, &entry.sha).unwrap().is_none());
    }

    #[test]
    fn missing_entry_is_reported() {
        let (dir, git) = repo();
        assert_eq!(
            pop_stash_entry_with(git, dir.path(), "0000", StashPopOptions::default()).unwrap(),
            StashPop::Missing
        );
    }
}
