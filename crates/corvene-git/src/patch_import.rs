//! Corvene (`1106-apply-patch`): apply a patch file or patch text, the
//! counterpart of Create Patch File ([`crate::format_patches`]). GHD can
//! write patches but has no way to apply one.
//!
//! [`preview_patch`] lists the files a patch touches (`git apply --numstat`
//! and `--summary`) and whether it applies as it is. A plain diff goes in
//! with [`apply_patch`]: straight onto the working copy when it applies
//! cleanly (local changes in the touched files are fine then), else with
//! `git apply --3way`, which can leave conflicts. A mailbox (`git
//! format-patch` output) can instead become commits with [`apply_mailbox`]
//! (`git am --3way`), which is undone (`git am --abort`) when any patch
//! fails, so nothing is left half applied.

use std::path::Path;
use std::sync::Arc;

use crate::clean::unquote_c_path;
use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

/// How a patch changes one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchFileChange {
    Modified,
    Added,
    Deleted,
    Renamed,
}

/// One file a patch touches (the counts summed over a mailbox's patches).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchFile {
    pub path: String,
    pub old_path: Option<String>,
    /// `None` for a binary change.
    pub added: Option<u32>,
    pub deleted: Option<u32>,
    pub change: PatchFileChange,
}

/// What [`preview_patch`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatchPreview {
    pub files: Vec<PatchFile>,
    /// A mailbox's patch subjects in order (`[PATCH n/m]` dropped); empty
    /// for a plain diff.
    pub commits: Vec<String>,
    /// `git apply --check` passed: no 3-way merge is needed.
    pub applies_cleanly: bool,
    /// Why it does not apply as it is (git's message), when it does not.
    pub check_error: Option<String>,
}

impl PatchPreview {
    pub fn is_mailbox(&self) -> bool {
        !self.commits.is_empty()
    }
}

/// What [`apply_patch`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchApply {
    Applied,
    /// The 3-way merge left these files conflicted.
    Conflicts(Vec<String>),
}

/// The files `patch` touches and whether it applies to `workdir` as it is.
/// An error means git found no patch in it.
pub fn preview_patch(git: Arc<GitBinary>, workdir: &Path, patch: &[u8]) -> Result<PatchPreview> {
    let apply = |args: &[&str]| {
        GitCommand::new(git.clone())
            .args(["-c", "core.quotePath=false", "apply"])
            .args(args)
            .arg("-")
            .current_dir(workdir)
            .stdin(patch.to_vec())
    };
    let numstat = apply(&["--numstat", "-z"]).run()?;
    let mut files = parse_numstat(&numstat.stdout);
    if files.is_empty() {
        return Err(GitError::Gix(
            "This does not look like a patch: git found no changes in it.".into(),
        ));
    }
    let summary = apply(&["--summary"]).run()?.stdout_string()?;
    for (change, path) in parse_summary(&summary) {
        if let Some(file) = files.iter_mut().find(|f| f.path == path) {
            file.change = change;
        }
    }
    let check = apply(&["--check", "--whitespace=nowarn"])
        .allow_any_exit_code()
        .run()?;
    let applies_cleanly = check.status.success();
    Ok(PatchPreview {
        files,
        commits: mailbox_subjects(&String::from_utf8_lossy(patch)),
        applies_cleanly,
        check_error: (!applies_cleanly).then(|| check.stderr.trim().to_string()),
    })
}

/// `git apply --numstat -z`: `added\tdeleted\tpath\0`, or for a rename
/// `added\tdeleted\t\0old\0new\0`; `-` counts for binary files. A path a
/// mailbox touches twice is listed once with the counts added up.
pub fn parse_numstat(stdout: &[u8]) -> Vec<PatchFile> {
    let mut fields = stdout
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned());
    let mut files: Vec<PatchFile> = Vec::new();
    while let Some(record) = fields.next() {
        if record.is_empty() {
            continue;
        }
        let mut cols = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        let (path, old_path) = if path.is_empty() {
            let (Some(old), Some(new)) = (fields.next(), fields.next()) else {
                break;
            };
            (new, Some(old))
        } else {
            (path.to_string(), None)
        };
        let added = added.parse::<u32>().ok();
        let deleted = deleted.parse::<u32>().ok();
        if let Some(file) = files.iter_mut().find(|f| f.path == path) {
            file.added = file.added.zip(added).map(|(a, b)| a + b);
            file.deleted = file.deleted.zip(deleted).map(|(a, b)| a + b);
            continue;
        }
        files.push(PatchFile {
            change: if old_path.is_some() {
                PatchFileChange::Renamed
            } else {
                PatchFileChange::Modified
            },
            path,
            old_path,
            added,
            deleted,
        });
    }
    files
}

/// `git apply --summary`'s ` create mode 100644 <path>` and ` delete mode
/// 100644 <path>` lines.
pub fn parse_summary(stdout: &str) -> Vec<(PatchFileChange, String)> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (change, rest) = if let Some(rest) = line.strip_prefix("create mode ") {
                (PatchFileChange::Added, rest)
            } else if let Some(rest) = line.strip_prefix("delete mode ") {
                (PatchFileChange::Deleted, rest)
            } else {
                return None;
            };
            let (_, path) = rest.split_once(' ')?;
            Some((change, unquote_c_path(path)))
        })
        .collect()
}

/// The subjects of a mailbox's patches (`From <sha> <date>` separators,
/// `Subject:` headers, folded lines joined, `[PATCH …]` dropped). Empty
/// when `text` is not a mailbox.
pub fn mailbox_subjects(text: &str) -> Vec<String> {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let starts_like_mail = first.starts_with("From ") || first.starts_with("From:");
    if !starts_like_mail {
        return Vec::new();
    }
    let mut subjects = Vec::new();
    let mut lines = text.lines().peekable();
    let mut in_headers = true;
    while let Some(line) = lines.next() {
        if line.starts_with("From ") && line.len() > 45 {
            in_headers = true;
            continue;
        }
        if in_headers && line.is_empty() {
            in_headers = false;
            continue;
        }
        if !in_headers {
            continue;
        }
        if let Some(subject) = line.strip_prefix("Subject:") {
            let mut subject = subject.trim().to_string();
            while let Some(next) = lines.peek() {
                if next.starts_with(' ') || next.starts_with('\t') {
                    subject.push(' ');
                    subject.push_str(next.trim());
                    lines.next();
                } else {
                    break;
                }
            }
            subjects.push(strip_patch_prefix(&subject).to_string());
        }
    }
    subjects
}

fn strip_patch_prefix(subject: &str) -> &str {
    let trimmed = subject.trim_start();
    if trimmed.starts_with('[')
        && let Some(end) = trimmed.find(']')
        && trimmed[1..end].to_ascii_uppercase().contains("PATCH")
    {
        return trimmed[end + 1..].trim_start();
    }
    trimmed
}

/// Apply `patch` to the working copy: as it is when it applies cleanly,
/// else with `git apply --3way` (which stages what it merged and leaves
/// the conflicted files unmerged).
pub fn apply_patch(git: Arc<GitBinary>, workdir: &Path, patch: &[u8]) -> Result<PatchApply> {
    let apply = |args: &[&str]| {
        GitCommand::new(git.clone())
            .arg("apply")
            .args(args)
            .args(["--whitespace=nowarn", "-"])
            .current_dir(workdir)
            .stdin(patch.to_vec())
    };
    if apply(&["--check"])
        .allow_any_exit_code()
        .run()?
        .status
        .success()
    {
        apply(&[]).run()?;
        return Ok(PatchApply::Applied);
    }
    let out = apply(&["--3way"]).allow_exit_code(1).run()?;
    if out.status.success() {
        return Ok(PatchApply::Applied);
    }
    let conflicted = crate::unmerged_paths(git.clone(), workdir)?;
    if conflicted.is_empty() {
        return Err(GitError::Failed {
            args: "apply --3way".into(),
            code: out.status.code(),
            stderr: out.stderr.trim().to_string(),
        });
    }
    Ok(PatchApply::Conflicts(conflicted))
}

/// `git am --3way` of mailbox `patch`: one commit per patch. When a patch
/// does not apply, the session is aborted (the branch and the files are as
/// they were) and the error says which patch failed.
pub fn apply_mailbox(git: Arc<GitBinary>, workdir: &Path, patch: &[u8]) -> Result<()> {
    let out = GitCommand::new(git.clone())
        .args(["am", "--3way", "--quiet"])
        .current_dir(workdir)
        .stdin(patch.to_vec())
        .allow_any_exit_code()
        .run()?;
    if out.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let message = [stdout.trim(), out.stderr.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let aborted = GitCommand::new(git)
        .args(["am", "--abort"])
        .current_dir(workdir)
        .allow_any_exit_code()
        .run()
        .is_ok_and(|o| o.status.success());
    Err(GitError::Gix(if aborted {
        format!(
            "The patches could not be committed, so nothing was changed. You can apply them as \
             uncommitted changes instead.\n\n{message}"
        )
    } else {
        format!("git am stopped and could not be undone; run git am --abort.\n\n{message}")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn numstat_records() {
        let out =
            b"3\t1\tsrc/main.rs\0-\t-\tlogo.png\x000\t0\t\0old.md\0new.md\x001\t0\tsrc/main.rs\0";
        let files = parse_numstat(out);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!((files[0].added, files[0].deleted), (Some(4), Some(1)));
        assert_eq!(files[1].added, None);
        assert_eq!(files[2].old_path.as_deref(), Some("old.md"));
        assert_eq!(files[2].change, PatchFileChange::Renamed);
    }

    #[test]
    fn summary_lines() {
        let out = " create mode 100644 CONTRIBUTING.md\n delete mode 100644 \"a\\tb\"\n mode change 100644 => 100755 run.sh\n";
        assert_eq!(
            parse_summary(out),
            vec![
                (PatchFileChange::Added, "CONTRIBUTING.md".to_string()),
                (PatchFileChange::Deleted, "a\tb".to_string()),
            ]
        );
    }

    #[test]
    fn mailbox_subjects_are_read() {
        let mbox = "From 1234567890123456789012345678901234567890 Mon Sep 17 00:00:00 2001\n\
                    From: A <a@example.com>\nSubject: [PATCH 1/2] Add a\n changelog\n\n\
                    body\n---\ndiff --git a/x b/x\n\
                    From abcdefabcdefabcdefabcdefabcdefabcdefabcd Mon Sep 17 00:00:00 2001\n\
                    Subject: Fix it\n\nbody\n";
        assert_eq!(mailbox_subjects(mbox), vec!["Add a changelog", "Fix it"]);
        assert!(mailbox_subjects("diff --git a/x b/x\n").is_empty());
    }

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path();
        run(p, &["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "T"),
            ("user.email", "t@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            run(p, &["config", k, v]);
        }
        std::fs::write(p.join("a.txt"), "1\n2\n3\n").expect("write");
        run(p, &["add", "-A"]);
        run(p, &["commit", "-q", "-m", "one"]);
        dir
    }

    #[test]
    fn previews_and_applies_with_local_changes() {
        let dir = repo();
        let p = dir.path();
        let git = Arc::new(crate::find_git().expect("git"));
        let patch = b"diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n 1\n-2\n+TWO\n 3\ndiff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+n\n";
        // a local change elsewhere in the file the patch touches
        std::fs::write(p.join("a.txt"), "1\n2\n3\n4\n").expect("write");
        let preview = preview_patch(git.clone(), p, patch).expect("preview");
        assert!(preview.applies_cleanly, "{preview:?}");
        assert!(!preview.is_mailbox());
        assert_eq!(preview.files[1].change, PatchFileChange::Added);
        assert_eq!(
            apply_patch(git, p, patch).expect("apply"),
            PatchApply::Applied
        );
        assert_eq!(
            std::fs::read_to_string(p.join("a.txt")).expect("read"),
            "1\nTWO\n3\n4\n"
        );
        assert!(p.join("n.txt").exists());
    }

    #[test]
    fn conflicting_patch_merges_three_way() {
        let dir = repo();
        let p = dir.path();
        let git = Arc::new(crate::find_git().expect("git"));
        std::fs::write(p.join("a.txt"), "1\nTWO\n3\n").expect("write");
        run(p, &["commit", "-qam", "two"]);
        let patch = run(p, &["format-patch", "-1", "--stdout"]);
        run(p, &["reset", "-q", "--hard", "HEAD~1"]);
        std::fs::write(p.join("a.txt"), "1\nzwei\n3\n").expect("write");
        run(p, &["commit", "-qam", "zwei"]);
        let preview = preview_patch(git.clone(), p, patch.as_bytes()).expect("preview");
        assert!(!preview.applies_cleanly);
        assert_eq!(preview.commits, vec!["two"]);
        assert!(apply_mailbox(git.clone(), p, patch.as_bytes()).is_err());
        assert!(!p.join(".git/rebase-apply").exists());
        assert_eq!(
            apply_patch(git, p, patch.as_bytes()).expect("apply"),
            PatchApply::Conflicts(vec!["a.txt".into()])
        );
    }
}
