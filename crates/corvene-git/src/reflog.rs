//! Corvene `1216-recent-activity`: the reflog of HEAD or a branch, read
//! in-process with gitoxide, each line's message parsed into what happened.
//! GitHub Desktop has no reflog view; it only reads the reflog for recent
//! branches and checkouts (`lib/git/reflog.ts`), through the git CLI.

use std::collections::HashSet;
use std::path::Path;

use crate::error::{GitError, Result};

/// One reflog line, newest first in [`read_reflog`]'s result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflogEntry {
    /// The ref's value before the change, empty when it was created.
    pub old: String,
    /// The ref's value after the change, empty when it was deleted.
    pub new: String,
    /// When it happened, seconds since the epoch and the UTC offset in
    /// seconds of whoever did it.
    pub seconds: i64,
    pub offset: i32,
    pub message: String,
    pub action: ReflogAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitKind {
    Normal,
    Initial,
    Amend,
    Merge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebaseStep {
    /// `rebase (start): checkout <onto>`
    Start,
    /// `rebase (pick|squash|fixup|reword|edit|continue)`: one commit replayed
    Pick,
    /// `rebase (finish): returning to refs/heads/<branch>`
    Finish,
    /// `rebase (abort): returning to …`
    Abort,
}

/// What a reflog message says happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReflogAction {
    /// `commit[ (initial|amend|merge)]: <summary>`
    Commit { kind: CommitKind, summary: String },
    /// `checkout: moving from <from> to <to>`
    Checkout { from: String, to: String },
    /// `reset: moving to <target>`
    Reset { target: String },
    /// `rebase[ -i] (<step>): <detail>`; `detail` is the onto revision for
    /// [`RebaseStep::Start`], the branch for Finish and Abort, the commit's
    /// summary for Pick.
    Rebase { step: RebaseStep, detail: String },
    /// `merge <what>: Fast-forward` or `merge <what>: Merge made by …`
    Merge { what: String, fast_forward: bool },
    /// `pull[ <args>]: Fast-forward`, `pull: Merge made by …`
    Pull { fast_forward: bool },
    /// `cherry-pick: <summary>`
    CherryPick { summary: String },
    /// `revert: <summary>`
    Revert { summary: String },
    /// `branch: Created from <start>`, `branch: Reset to <rev>`, …
    Branch { detail: String },
    /// `clone: from <url>`
    Clone { url: String },
    /// Anything else, shown as the raw message.
    Other,
}

/// What `message` (a reflog line's message) says happened.
pub fn parse_action(message: &str) -> ReflogAction {
    let Some((op, rest)) = message.split_once(": ") else {
        return ReflogAction::Other;
    };
    let rest = rest.trim();
    // `rebase -i (start)` and git's older `rebase -m (pick)` spell the mode
    let (verb, paren) = match op.find(" (") {
        Some(i) if op.ends_with(')') => (&op[..i], Some(&op[i + 2..op.len() - 1])),
        _ => (op, None),
    };
    let verb = verb
        .strip_suffix(" -i")
        .or_else(|| verb.strip_suffix(" -m"))
        .unwrap_or(verb);
    match (verb, paren) {
        ("commit", kind) => ReflogAction::Commit {
            kind: match kind {
                Some("initial") => CommitKind::Initial,
                Some("amend") => CommitKind::Amend,
                Some("merge") => CommitKind::Merge,
                _ => CommitKind::Normal,
            },
            summary: rest.to_string(),
        },
        ("checkout", None) => match rest
            .strip_prefix("moving from ")
            .and_then(|r| r.split_once(" to "))
        {
            Some((from, to)) => ReflogAction::Checkout {
                from: from.to_string(),
                to: to.to_string(),
            },
            None => ReflogAction::Other,
        },
        ("reset", None) => ReflogAction::Reset {
            target: rest.strip_prefix("moving to ").unwrap_or(rest).to_string(),
        },
        ("rebase", Some(step)) => {
            let (step, detail) = match step {
                "start" => (
                    RebaseStep::Start,
                    rest.strip_prefix("checkout ").unwrap_or(rest),
                ),
                "finish" => (RebaseStep::Finish, returning_to(rest)),
                "abort" => (RebaseStep::Abort, returning_to(rest)),
                _ => (RebaseStep::Pick, rest),
            };
            ReflogAction::Rebase {
                step,
                detail: detail.to_string(),
            }
        }
        // git before 2.26 (`git-rebase--am`): `rebase: <summary>` per commit,
        // `rebase finished: returning to refs/heads/<branch>`
        ("rebase", None) => ReflogAction::Rebase {
            step: RebaseStep::Pick,
            detail: rest.to_string(),
        },
        ("rebase finished", None) => ReflogAction::Rebase {
            step: RebaseStep::Finish,
            detail: returning_to(rest).to_string(),
        },
        ("pull", _) => ReflogAction::Pull {
            fast_forward: rest == "Fast-forward",
        },
        (v, None) if v.starts_with("pull ") => ReflogAction::Pull {
            fast_forward: rest == "Fast-forward",
        },
        (v, None) if v.starts_with("merge ") => ReflogAction::Merge {
            what: v["merge ".len()..].to_string(),
            fast_forward: rest == "Fast-forward",
        },
        ("cherry-pick", _) => ReflogAction::CherryPick {
            summary: rest.to_string(),
        },
        ("revert", _) => ReflogAction::Revert {
            summary: rest.to_string(),
        },
        ("branch", None) => ReflogAction::Branch {
            detail: rest.to_string(),
        },
        ("clone", None) => ReflogAction::Clone {
            url: rest.strip_prefix("from ").unwrap_or(rest).to_string(),
        },
        _ => ReflogAction::Other,
    }
}

/// `returning to refs/heads/main` → `main`
fn returning_to(rest: &str) -> &str {
    let r = rest.strip_prefix("returning to ").unwrap_or(rest);
    r.strip_prefix("refs/heads/").unwrap_or(r)
}

fn hex_or_empty(id: &gix::ObjectId) -> String {
    if id.is_null() {
        String::new()
    } else {
        id.to_string()
    }
}

/// The reflog of `refname` (`HEAD` or a full ref name such as
/// `refs/heads/main`), newest first, at most `limit` lines. Empty when the
/// ref or its log does not exist. Lines that do not parse are skipped.
pub fn read_reflog(workdir: &Path, refname: &str, limit: usize) -> Result<Vec<ReflogEntry>> {
    let repo = crate::handle::open(workdir)?;
    let Ok(Some(reference)) = repo.try_find_reference(refname) else {
        return Ok(Vec::new());
    };
    let mut platform = reference.log_iter();
    let Some(lines) = platform.rev().map_err(GitError::Io)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for line in lines.flatten().take(limit) {
        let message = line.message.to_string();
        out.push(ReflogEntry {
            old: hex_or_empty(&line.previous_oid),
            new: hex_or_empty(&line.new_oid),
            seconds: line.signature.time.seconds,
            offset: line.signature.time.offset,
            action: parse_action(&message),
            message,
        });
    }
    Ok(out)
}

/// Which of `candidates` (commit SHAs) can be reached from HEAD, a local
/// or remote-tracking branch, or a tag. The walk stops as soon as every
/// candidate has been seen, so it only goes through the whole history when
/// something is unreachable.
pub fn reachable_shas(workdir: &Path, candidates: &[String]) -> Result<HashSet<String>> {
    let repo = crate::handle::open(workdir)?;
    let wanted: HashSet<gix::ObjectId> = candidates
        .iter()
        .filter_map(|c| gix::ObjectId::from_hex(c.as_bytes()).ok())
        .collect();
    let mut found: HashSet<String> = HashSet::new();
    if wanted.is_empty() {
        return Ok(found);
    }
    let mut tips: Vec<gix::ObjectId> = Vec::new();
    if let Ok(head) = repo.head_id() {
        tips.push(head.detach());
    }
    let refs = repo
        .references()
        .map_err(|e| GitError::Gix(e.to_string()))?;
    for iter in [refs.local_branches(), refs.remote_branches(), refs.tags()] {
        let iter = iter.map_err(|e| GitError::Gix(e.to_string()))?;
        for mut r in iter.flatten() {
            // a tag can point at a tree or blob
            if let Ok(id) = r.peel_to_id()
                && let Ok(object) = id.object()
                && object.kind == gix::object::Kind::Commit
                && !tips.contains(&object.id)
            {
                tips.push(object.id);
            }
        }
    }
    if tips.is_empty() {
        return Ok(found);
    }
    let walk = repo
        .rev_walk(tips)
        .all()
        .map_err(|e| GitError::Gix(e.to_string()))?;
    for info in walk {
        let Ok(info) = info else {
            // a missing parent (shallow clone) ends that line only
            continue;
        };
        if wanted.contains(&info.id) {
            found.insert(info.id.to_string());
            if found.len() == wanted.len() {
                break;
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn parses_messages() {
        use ReflogAction::*;
        let cases: &[(&str, ReflogAction)] = &[
            (
                "commit: Fix it",
                Commit {
                    kind: CommitKind::Normal,
                    summary: "Fix it".into(),
                },
            ),
            (
                "commit (initial): First",
                Commit {
                    kind: CommitKind::Initial,
                    summary: "First".into(),
                },
            ),
            (
                "commit (amend): Fix it",
                Commit {
                    kind: CommitKind::Amend,
                    summary: "Fix it".into(),
                },
            ),
            (
                "checkout: moving from main to feature/x",
                Checkout {
                    from: "main".into(),
                    to: "feature/x".into(),
                },
            ),
            (
                "reset: moving to HEAD~3",
                Reset {
                    target: "HEAD~3".into(),
                },
            ),
            (
                "rebase (start): checkout main",
                Rebase {
                    step: RebaseStep::Start,
                    detail: "main".into(),
                },
            ),
            (
                "rebase -i (start): checkout abc123",
                Rebase {
                    step: RebaseStep::Start,
                    detail: "abc123".into(),
                },
            ),
            (
                "rebase (pick): Add a thing",
                Rebase {
                    step: RebaseStep::Pick,
                    detail: "Add a thing".into(),
                },
            ),
            (
                "rebase -i (finish): returning to refs/heads/topic",
                Rebase {
                    step: RebaseStep::Finish,
                    detail: "topic".into(),
                },
            ),
            (
                "rebase (abort): returning to refs/heads/topic",
                Rebase {
                    step: RebaseStep::Abort,
                    detail: "topic".into(),
                },
            ),
            (
                "rebase finished: returning to refs/heads/topic",
                Rebase {
                    step: RebaseStep::Finish,
                    detail: "topic".into(),
                },
            ),
            (
                "merge feature: Fast-forward",
                Merge {
                    what: "feature".into(),
                    fast_forward: true,
                },
            ),
            (
                "merge origin/main: Merge made by the 'ort' strategy.",
                Merge {
                    what: "origin/main".into(),
                    fast_forward: false,
                },
            ),
            ("pull: Fast-forward", Pull { fast_forward: true }),
            (
                "pull --no-rebase origin main: Merge made by the 'ort' strategy.",
                Pull {
                    fast_forward: false,
                },
            ),
            (
                "cherry-pick: Port it",
                CherryPick {
                    summary: "Port it".into(),
                },
            ),
            (
                "revert: Revert \"x\"",
                Revert {
                    summary: "Revert \"x\"".into(),
                },
            ),
            (
                "branch: Created from HEAD",
                Branch {
                    detail: "Created from HEAD".into(),
                },
            ),
            (
                "clone: from https://github.com/a/b.git",
                Clone {
                    url: "https://github.com/a/b.git".into(),
                },
            ),
            ("update by push", Other),
        ];
        for (message, want) in cases {
            assert_eq!(&parse_action(message), want, "{message}");
        }
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn repo(commits: usize) -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q", "-b", "main"]);
        git(p, &["config", "commit.gpgsign", "false"]);
        let mut shas = Vec::new();
        for i in 1..=commits {
            std::fs::write(p.join("f.txt"), format!("{i}\n")).unwrap();
            git(p, &["add", "."]);
            git(p, &["commit", "-q", "-m", &format!("c{i}")]);
            shas.push(git(p, &["rev-parse", "HEAD"]));
        }
        (dir, shas)
    }

    #[test]
    fn hard_reset_leaves_unreachable_commits() {
        let (dir, shas) = repo(5);
        git(dir.path(), &["reset", "-q", "--hard", "HEAD~3"]);
        let log = read_reflog(dir.path(), "HEAD", 100).unwrap();
        assert_eq!(log.len(), 6);
        assert_eq!(
            log[0].action,
            ReflogAction::Reset {
                target: "HEAD~3".into()
            }
        );
        assert_eq!(log[0].old, shas[4]);
        assert_eq!(log[0].new, shas[1]);
        assert!(matches!(
            log[5].action,
            ReflogAction::Commit {
                kind: CommitKind::Initial,
                ..
            }
        ));
        assert!(log[5].old.is_empty());

        let all: Vec<String> = log.iter().map(|e| e.new.clone()).collect();
        let reachable = reachable_shas(dir.path(), &all).unwrap();
        let expected: HashSet<String> = shas[..2].iter().cloned().collect();
        assert_eq!(reachable, expected);

        let commits = crate::log::commits_by_sha(dir.path(), &all).unwrap();
        assert_eq!(commits.len(), 5);
        assert_eq!(commits[0].sha, shas[1]);
    }

    #[test]
    fn deleted_branch_tip_stays_in_head_log() {
        let (dir, _) = repo(1);
        let p = dir.path();
        git(p, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(p.join("g.txt"), "g\n").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "-m", "on feature"]);
        let tip = git(p, &["rev-parse", "HEAD"]);
        git(p, &["checkout", "-q", "main"]);
        git(p, &["branch", "-q", "-D", "feature"]);

        assert!(read_reflog(p, "refs/heads/feature", 10).unwrap().is_empty());
        let log = read_reflog(p, "HEAD", 10).unwrap();
        assert_eq!(
            log[0].action,
            ReflogAction::Checkout {
                from: "feature".into(),
                to: "main".into()
            }
        );
        assert_eq!(log[0].old, tip);
        assert!(reachable_shas(p, &[tip]).unwrap().is_empty());
        assert_eq!(read_reflog(p, "HEAD", 2).unwrap().len(), 2);
    }

    #[test]
    fn unborn_repository_has_no_log() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        assert!(read_reflog(dir.path(), "HEAD", 10).unwrap().is_empty());
    }
}
