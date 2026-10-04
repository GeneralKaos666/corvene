//! Staging, committing, discarding and undoing - GHD `lib/git/{update-index,
//! reset,commit,checkout-index}.ts`, `app-store._commitIncludedChanges` and
//! `GitStore.discardChanges` / `undoCommit` (`lib/stores/git-store.ts`).
//!
//! Deviation: [`discard_changes`] resets paths without naming `HEAD`, so it
//! also works on an unborn branch.

use std::path::Path;
use std::sync::Arc;

use corvene_models::{DiffSelectionType, FileStatusKind, WorkingDirectoryFileChange};
use tracing::info;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

#[derive(Clone, Debug, Default)]
pub struct CommitOptions {
    pub amend: bool,
    pub no_verify: bool,
    pub signoff: bool,
    pub allow_empty: bool,
}

/// `git reset -- .` (GHD `unstageAll`). On an unborn branch there is no HEAD
/// to reset to, so fall back to clearing the index.
pub fn unstage_all(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    let out = GitCommand::new(git.clone())
        .args(["reset", "--", "."])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        GitCommand::new(git)
            .args(["rm", "-r", "--cached", "-q", "--ignore-unmatch", "--", "."])
            .current_dir(workdir)
            .allow_exit_code(128)
            .run()?;
    }
    Ok(())
}

/// GHD `stageFiles` (`lib/git/update-index.ts`) for every fully-included
/// file, in three `update-index` steps: force-remove the old paths of
/// renames (a new file may sit at the old path and stay out of the commit),
/// add the paths themselves, then force-remove the deleted files (one still
/// on disk, e.g. after `git rm --cached`, is committed as deleted).
/// Partially-selected files are staged separately by `stage_partial_files`
/// (`apply --cached`).
pub fn stage_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> Result<()> {
    stage_whole_files(
        git,
        workdir,
        files
            .iter()
            .filter(|f| f.selection.kind() == DiffSelectionType::All),
    )
}

/// The `update-index` steps of [`stage_files`] for `files`, whatever their
/// selection.
pub(crate) fn stage_whole_files<'a>(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: impl IntoIterator<Item = &'a WorkingDirectoryFileChange>,
) -> Result<()> {
    let mut normal: Vec<&str> = Vec::new();
    let mut old_renamed: Vec<&str> = Vec::new();
    let mut deleted: Vec<&str> = Vec::new();
    for file in files {
        normal.push(&file.path);
        match file.status.kind {
            FileStatusKind::Renamed => old_renamed.extend(file.old_path.as_deref()),
            FileStatusKind::Deleted => deleted.push(&file.path),
            _ => {}
        }
    }
    update_index(git.clone(), workdir, &old_renamed, true)?;
    update_index(git.clone(), workdir, &normal, false)?;
    update_index(git, workdir, &deleted, true)
}

/// GHD `updateIndex`: `update-index --add --remove [--force-remove]
/// --replace -z --stdin` for `paths`; nothing without paths.
fn update_index(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[&str],
    force_remove: bool,
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["update-index", "--add", "--remove"];
    if force_remove {
        args.push("--force-remove");
    }
    args.extend(["--replace", "-z", "--stdin"]);
    GitCommand::new(git)
        .args(args)
        .current_dir(workdir)
        .stdin(nul_separated(paths))
        .run()?;
    Ok(())
}

/// Corvene `715-assume-unchanged`: `update-index --[no-]assume-unchanged`
/// for tracked `paths`, so git stops (or resumes) reporting their changes.
pub fn set_assume_unchanged(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
    assume: bool,
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut stdin: Vec<u8> = Vec::new();
    for path in paths {
        stdin.extend_from_slice(path.as_bytes());
        stdin.push(0);
    }
    GitCommand::new(git)
        .args([
            "update-index",
            if assume {
                "--assume-unchanged"
            } else {
                "--no-assume-unchanged"
            },
            "-z",
            "--stdin",
        ])
        .current_dir(workdir)
        .stdin(stdin)
        .run()?;
    Ok(())
}

/// Paths marked assume-unchanged (`ls-files -v`: a lower-case tag).
pub fn assume_unchanged_paths(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<String>> {
    let out = GitCommand::new(git)
        .args(["ls-files", "-v", "-z"])
        .current_dir(workdir)
        .run()?;
    Ok(out
        .stdout
        .split(|&b| b == 0)
        .filter_map(|entry| {
            let (tag, path) = (entry.first()?, entry.get(2..)?);
            tag.is_ascii_lowercase()
                .then(|| String::from_utf8_lossy(path).into_owned())
        })
        .collect())
}

/// `git add -- <paths>` (the tutorial repository's README).
pub fn add_paths(git: Arc<GitBinary>, workdir: &Path, paths: &[&str]) -> Result<()> {
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    GitCommand::new(git).args(args).current_dir(workdir).run()?;
    Ok(())
}

/// GHD `parseCommitSHA` (`lib/git/core.ts`): the second word of `git
/// commit`'s `[<branch> <sha>] <summary>` line, which is the abbreviated
/// sha, or `(root-commit)` for a branch's first commit.
pub fn parse_commit_sha(stdout: &str) -> String {
    let head = stdout.split(']').next().unwrap_or_default();
    head.split(' ').nth(1).unwrap_or_default().to_string()
}

/// `git commit -F -` with the message on stdin (GHD `createCommit`).
/// Returns [`parse_commit_sha`] of git's output, as GHD does; callers that
/// need the full sha read [`head_sha`].
pub fn commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    message: &str,
    opts: &CommitOptions,
) -> Result<String> {
    let mut args = vec!["commit", "-F", "-"];
    if opts.amend {
        args.push("--amend");
    }
    if opts.no_verify {
        args.push("--no-verify");
    }
    if opts.signoff {
        args.push("--signoff");
    }
    if opts.allow_empty {
        args.push("--allow-empty");
    }
    let out = GitCommand::new(git)
        .args(args)
        .current_dir(workdir)
        .stdin(message.as_bytes().to_vec())
        .run()?;
    let sha = parse_commit_sha(&String::from_utf8_lossy(&out.stdout));
    info!(%sha, "created commit");
    Ok(sha)
}

pub fn head_sha(git: Arc<GitBinary>, workdir: &Path) -> Result<String> {
    // in-process (a `git rev-parse` spawn otherwise)
    if let Some(id) = crate::handle::open(workdir)
        .ok()
        .and_then(|repo| repo.head_id().ok().map(|id| id.detach()))
    {
        return Ok(id.to_string());
    }
    let out = GitCommand::new(git)
        .args(["rev-parse", "HEAD"])
        .current_dir(workdir)
        .run()?;
    Ok(out.stdout_string()?.trim().to_string())
}

/// GHD `mergeTrailers`: `git interpret-trailers --no-divider --trailer k=v …`
/// appends the trailers to a commit message (folding into an existing
/// trailer block when there is one).
pub fn merge_trailers(
    git: Arc<GitBinary>,
    workdir: &Path,
    message: &str,
    trailers: &[(String, String)],
) -> Result<String> {
    if trailers.is_empty() {
        return Ok(message.to_string());
    }
    let mut cmd = GitCommand::new(git)
        .args(["interpret-trailers", "--no-divider"])
        .current_dir(workdir);
    for (token, value) in trailers {
        cmd = cmd.arg("--trailer").arg(format!("{token}={value}"));
    }
    let out = cmd.stdin(message.as_bytes().to_vec()).run()?;
    out.stdout_string()
}

/// Summary + blank line + description, as GHD formats the message.
pub fn format_message(summary: &str, description: &str) -> String {
    let summary = summary.trim();
    let description = description.trim();
    if description.is_empty() {
        format!("{summary}\n")
    } else {
        format!("{summary}\n\n{description}\n")
    }
}

/// GHD `GitStore.undoCommit` for the `HEAD` commit: `git reset <parent>`
/// (mixed), so its changes stay in the working directory, unstaged. A root
/// commit (GHD `undoFirstCommit`): files deleted from the working directory
/// are checked out again (they would be lost with the commit), the branch's
/// HEAD ref is deleted and the index is cleared, leaving every file
/// untracked.
pub fn undo_last_commit(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    let parent = GitCommand::new(git.clone())
        .args(["rev-parse", "--verify", "--quiet", "HEAD^"])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()?;
    if parent.status.success() {
        let parent = parent.stdout_string()?.trim().to_string();
        GitCommand::new(git)
            .args(["reset", &parent])
            .current_dir(workdir)
            .run()?;
    } else {
        let status = crate::status::get_status(git.clone(), workdir)?;
        let deleted: Vec<&str> = status
            .files
            .iter()
            .filter(|f| f.status.kind == FileStatusKind::Deleted)
            .map(|f| f.path.as_str())
            .collect();
        if !deleted.is_empty() {
            GitCommand::new(git.clone())
                .args(["checkout", "HEAD", "--"])
                .args(&deleted)
                .current_dir(workdir)
                .run()?;
        }
        GitCommand::new(git.clone())
            .args(["update-ref", "-d", "HEAD", "-m", "Reverting first commit"])
            .current_dir(workdir)
            .run()?;
        // keep the files: clear the index back to "unstaged"
        unstage_all(git, workdir)?;
    }
    Ok(())
}

/// Discard working-directory changes (GHD `GitStore.discardChanges`):
///
/// 1. every file that is not deleted (nor a submodule) goes to the Trash
///    (`moveToTrash`, so a discard is recoverable), or is deleted when
///    `move_to_trash` is off or the Trash fails;
/// 2. submodules are reset to their recorded commit (`submodule update
///    --recursive --force`);
/// 3. the discarded paths that differ between the index and `HEAD` are reset
///    (`git reset -- <paths>`), so other staged files stay staged;
/// 4. the discarded paths are checked out from the index (`checkout-index`;
///    exit code 1, for paths that are not in it, is fine). A copy or rename
///    resets its new path and checks out (and resets) its old one.
///
/// Corvene resets the paths without naming `HEAD` (GHD `reset HEAD --`), so
/// a staged new file can be discarded on an unborn branch too.
///
/// With `clean_submodules` (Corvene, flag `discard-submodule-changes`), a
/// submodule entry with changes inside also has its modified files checked
/// out and its untracked (not ignored) files moved to the Trash, so the entry
/// goes away; GHD leaves a submodule's untracked files.
pub fn discard_changes(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    move_to_trash: bool,
    clean_submodules: bool,
) -> Result<()> {
    if clean_submodules {
        for file in files {
            if let Some(sub) = file.status.submodule_status {
                discard_inside_submodule(
                    git.clone(),
                    &workdir.join(&file.path),
                    sub,
                    move_to_trash,
                )?;
            }
        }
    }
    let mut to_checkout: Vec<&str> = Vec::new();
    let mut to_reset: Vec<&str> = Vec::new();
    let mut submodules: Vec<&str> = Vec::new();
    for file in files {
        if file.status.submodule {
            submodules.push(&file.path);
        } else if file.status.kind != FileStatusKind::Deleted {
            let full = workdir.join(file.path.trim_end_matches('/'));
            if !move_to_trash || !trashed(&full) {
                let _ = std::fs::remove_file(&full).or_else(|_| std::fs::remove_dir_all(&full));
            }
        }
        match (file.status.kind, file.old_path.as_deref()) {
            (FileStatusKind::Copied | FileStatusKind::Renamed, Some(old)) => {
                // the new path is gone already; the index must forget it
                to_reset.push(&file.path);
                to_checkout.push(old);
                to_reset.push(old);
            }
            _ => {
                to_checkout.push(&file.path);
                to_reset.push(&file.path);
            }
        }
    }
    let index_changes = index_changes(git.clone(), workdir)?;
    let to_reset: Vec<&str> = to_reset
        .into_iter()
        .filter(|p| index_changes.contains_key(*p))
        .collect();
    let submodule_paths: Vec<&str> = to_checkout
        .iter()
        .copied()
        .filter(|p| submodules.contains(p))
        .collect();
    // GHD's filter: only a submodule that was added in the index is left out
    let to_checkout: Vec<&str> = to_checkout
        .into_iter()
        .filter(|p| !submodule_paths.contains(p) || index_changes.get(*p) != Some(&'A'))
        .collect();
    crate::submodule::reset_submodule_paths(git.clone(), workdir, &submodule_paths)?;
    if !to_reset.is_empty() {
        GitCommand::new(git.clone())
            .args([
                "reset",
                "-q",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ])
            .env("GIT_LITERAL_PATHSPECS", "1")
            .current_dir(workdir)
            .stdin(nul_separated(&to_reset))
            .run()?;
    }
    if !to_checkout.is_empty() {
        GitCommand::new(git)
            .args(["checkout-index", "-f", "-u", "-q", "--stdin", "-z"])
            .current_dir(workdir)
            .stdin(nul_separated(&to_checkout))
            .allow_exit_code(1)
            .run()?;
    }
    Ok(())
}

fn nul_separated(paths: &[&str]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for path in paths {
        out.extend_from_slice(path.as_bytes());
        out.push(0);
    }
    out
}

/// GHD `getIndexChanges` (`lib/git/diff-index.ts`): the paths whose index
/// entry differs from `HEAD` (the empty tree on an unborn branch), with
/// their `--name-status` letter.
fn index_changes(
    git: Arc<GitBinary>,
    workdir: &Path,
) -> Result<std::collections::HashMap<String, char>> {
    let run = |base: &str| {
        GitCommand::new(git.clone())
            .args([
                "diff-index",
                "--cached",
                "--name-status",
                "--no-renames",
                "-z",
                base,
                "--",
            ])
            .current_dir(workdir)
            .allow_exit_code(128)
            .run()
    };
    let mut out = run("HEAD")?;
    if out.status.code() == Some(128) {
        out = run(crate::log::NULL_TREE_SHA)?;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let pieces: Vec<&str> = text.split('\0').collect();
    let (pairs, _) = pieces.as_chunks::<2>();
    Ok(pairs
        .iter()
        .filter_map(|[status, path]| Some((path.to_string(), status.chars().next()?)))
        .collect())
}

/// Whether `path` went to the Trash. Android has none, so the caller deletes.
fn trashed(path: &Path) -> bool {
    #[cfg(not(target_os = "android"))]
    {
        trash::delete(path).is_ok()
    }
    #[cfg(target_os = "android")]
    {
        let _ = path;
        false
    }
}

/// Check out a submodule's modified files and trash its untracked ones.
fn discard_inside_submodule(
    git: Arc<GitBinary>,
    submodule: &Path,
    status: corvene_models::SubmoduleStatus,
    move_to_trash: bool,
) -> Result<()> {
    if !submodule.is_dir() {
        return Ok(());
    }
    if status.modified_changes {
        GitCommand::new(git.clone())
            .args(["checkout", "-f", "-q", "--", "."])
            .current_dir(submodule)
            .run()?;
    }
    if status.untracked_changes {
        let out = GitCommand::new(git)
            .args([
                "ls-files",
                "--others",
                "--exclude-standard",
                "--directory",
                "-z",
            ])
            .current_dir(submodule)
            .run()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for path in text.split('\0').filter(|p| !p.is_empty()) {
            let full = submodule.join(path.trim_end_matches('/'));
            if !move_to_trash || !trashed(&full) {
                let _ = std::fs::remove_file(&full).or_else(|_| std::fs::remove_dir_all(&full));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(dir.path())
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        // Git for Windows checks text out with CRLF unless told not to
        run(&["config", "core.autocrlf", "false"]);
        run(&["config", "commit.gpgsign", "false"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@example.com"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    #[test]
    fn stage_commit_undo_round_trip() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        std::fs::write(path.join("b.txt"), "two\n").unwrap();
        let mut status = crate::get_status(git.clone(), path).unwrap();
        // exclude b.txt
        for f in &mut status.files {
            if f.path == "b.txt" {
                f.selection = corvene_models::DiffSelection::none();
            }
        }
        unstage_all(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        let parsed = commit(
            git.clone(),
            path,
            &format_message("Add a", "details"),
            &CommitOptions::default(),
        )
        .unwrap();
        // GHD `parseCommitSHA` of a branch's first commit
        assert_eq!(parsed, "(root-commit)");
        let sha = head_sha(git.clone(), path).unwrap();
        let after = crate::get_status(git.clone(), path).unwrap();
        let paths: Vec<_> = after.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["b.txt"]);
        let log = Command::new("git")
            .args(["log", "-1", "--format=%B"])
            .current_dir(path)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim(),
            "Add a\n\ndetails"
        );

        // second commit then undo keeps changes
        std::fs::write(path.join("a.txt"), "changed\n").unwrap();
        let status = crate::get_status(git.clone(), path).unwrap();
        unstage_all(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "second\n", &CommitOptions::default()).unwrap();
        undo_last_commit(git.clone(), path).unwrap();
        let head = head_sha(git.clone(), path).unwrap();
        assert_eq!(head, sha);
        let status = crate::get_status(git.clone(), path).unwrap();
        assert!(status.files.iter().any(|f| f.path == "a.txt"));

        // undo the root commit: files stay, HEAD unborn
        undo_last_commit(git.clone(), path).unwrap();
        let info = crate::open_repository(path).unwrap();
        assert!(matches!(info.tip, corvene_models::Tip::Unborn { .. }));
        assert!(path.join("a.txt").exists());
    }

    #[test]
    fn parses_the_commit_sha_like_ghd() {
        assert_eq!(
            parse_commit_sha("[main 1a2b3c4] Add a\n 1 file changed\n"),
            "1a2b3c4"
        );
        assert_eq!(
            parse_commit_sha("[main (root-commit) 1a2b3c4] Add a\n"),
            "(root-commit)"
        );
        assert_eq!(parse_commit_sha(""), "");
    }

    #[test]
    fn assume_unchanged_round_trip() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        std::fs::write(path.join("b c.txt"), "two\n").unwrap();
        let status = crate::get_status(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "init\n", &CommitOptions::default()).unwrap();
        std::fs::write(path.join("a.txt"), "dirty\n").unwrap();
        std::fs::write(path.join("b c.txt"), "dirty\n").unwrap();
        let both = vec!["a.txt".to_string(), "b c.txt".to_string()];
        set_assume_unchanged(git.clone(), path, &both, true).unwrap();
        assert_eq!(assume_unchanged_paths(git.clone(), path).unwrap(), both);
        let status = crate::get_status(git.clone(), path).unwrap();
        assert!(status.files.is_empty());
        set_assume_unchanged(git.clone(), path, &both, false).unwrap();
        assert!(
            assume_unchanged_paths(git.clone(), path)
                .unwrap()
                .is_empty()
        );
        let status = crate::get_status(git, path).unwrap();
        assert_eq!(status.files.len(), 2);
    }

    #[test]
    fn discard_restores_and_removes() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        let status = crate::get_status(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "init\n", &CommitOptions::default()).unwrap();
        std::fs::write(path.join("a.txt"), "dirty\n").unwrap();
        std::fs::write(path.join("new.txt"), "x\n").unwrap();
        let status = crate::get_status(git.clone(), path).unwrap();
        discard_changes(git.clone(), path, &status.files, false, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "one\n"
        );
        assert!(!path.join("new.txt").exists());
        let status = crate::get_status(git, path).unwrap();
        assert!(status.files.is_empty());
    }

    #[test]
    fn discard_cleans_inside_a_submodule() {
        let (sub_dir, git) = repo();
        let git_in = |dir: &Path, args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(["-c", "protocol.file.allow=always"])
                    .args(args)
                    .current_dir(dir)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        std::fs::write(sub_dir.path().join("lib.txt"), "lib\n").unwrap();
        git_in(sub_dir.path(), &["add", "."]);
        git_in(sub_dir.path(), &["commit", "-q", "-m", "lib"]);
        let (dir, _) = repo();
        let path = dir.path();
        let url = sub_dir.path().to_string_lossy().into_owned();
        git_in(path, &["submodule", "add", "-q", &url, "sub"]);
        git_in(path, &["commit", "-q", "-m", "add sub"]);
        let sub = path.join("sub");
        std::fs::write(sub.join("lib.txt"), "dirty\n").unwrap();
        std::fs::write(sub.join("junk.txt"), "x\n").unwrap();
        std::fs::create_dir(sub.join("junkdir")).unwrap();
        std::fs::write(sub.join("junkdir/more.txt"), "y\n").unwrap();
        let status = crate::get_status(git.clone(), path).unwrap();
        assert_eq!(status.files.len(), 1);
        let hidden = crate::get_status_with(
            git.clone(),
            path,
            crate::StatusOptions {
                ignore_submodules: crate::IgnoreSubmodules::Dirty,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(hidden.files.is_empty());
        // GHD behaviour: `submodule update --force` checks the recorded
        // commit out again, the untracked files stay
        discard_changes(git.clone(), path, &status.files, false, false).unwrap();
        assert!(sub.join("junk.txt").exists());
        assert_eq!(
            std::fs::read_to_string(sub.join("lib.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "lib\n"
        );
        std::fs::write(sub.join("lib.txt"), "dirty\n").unwrap();
        discard_changes(git.clone(), path, &status.files, false, true).unwrap();
        // the submodule's clone has the machine's `core.autocrlf`
        assert_eq!(
            std::fs::read_to_string(sub.join("lib.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "lib\n"
        );
        assert!(!sub.join("junk.txt").exists());
        assert!(!sub.join("junkdir").exists());
        let status = crate::get_status(git, path).unwrap();
        assert!(status.files.is_empty());
    }
}
