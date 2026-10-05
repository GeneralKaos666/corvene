//! `git bisect` (Corvene `1212-bisect`; GitHub Desktop has no bisect).
//!
//! The state is read from the git directory (`BISECT_START`,
//! `BISECT_TERMS`, `BISECT_NAMES`, `BISECT_FIRST_PARENT`) and
//! `git for-each-ref refs/bisect/`, so a bisect started on the command line
//! reads the same as one started in Corvene. Every change runs `git bisect`.

use std::path::Path;
use std::sync::Arc;

use corvene_models::BisectState;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// What a commit is marked as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BisectVerdict {
    Good,
    Bad,
    Skip,
}

/// A bisect is in progress (`BISECT_START` exists): one stat, for callers
/// that only need to know whether to read [`bisect_state`].
pub fn bisect_in_progress(workdir: &Path) -> bool {
    crate::paths::git_dir(workdir)
        .join("BISECT_START")
        .is_file()
}

/// The bisect in progress, `None` without one.
pub fn bisect_state(git: Arc<GitBinary>, workdir: &Path) -> Result<Option<BisectState>> {
    let dir = crate::paths::git_dir(workdir);
    let Ok(start) = std::fs::read_to_string(dir.join("BISECT_START")) else {
        return Ok(None);
    };
    let (term_bad, term_good) = read_terms(&dir);
    // refs/bisect/ is per worktree; git finds it in this one's git directory
    // whatever the ref storage
    let out = GitCommand::new(git)
        .args([
            "for-each-ref",
            "--format=%(objectname) %(refname)",
            "refs/bisect/",
        ])
        .current_dir(workdir)
        .run()?;
    Ok(Some(parse_refs(
        &String::from_utf8_lossy(&out.stdout),
        BisectState {
            start: start.trim().to_string(),
            term_bad,
            term_good,
            ..Default::default()
        },
    )))
}

/// `for-each-ref` lines (`<sha> refs/bisect/<name>`) into `state`.
fn parse_refs(stdout: &str, mut state: BisectState) -> BisectState {
    for line in stdout.lines() {
        let Some((sha, name)) = line.trim().split_once(' ') else {
            continue;
        };
        let Some(name) = name.strip_prefix("refs/bisect/") else {
            continue;
        };
        if name == state.term_bad {
            state.bad = Some(sha.to_string());
        } else if name
            .strip_prefix(state.term_good.as_str())
            .is_some_and(|rest| rest.starts_with('-'))
        {
            state.good.push(sha.to_string());
        } else if name.starts_with("skip-") {
            state.skipped.push(sha.to_string());
        }
    }
    state
}

/// `BISECT_TERMS` (the bad term, then the good one); git writes it at the
/// first mark, so a bisect without one uses `bad` and `good`.
fn read_terms(git_dir: &Path) -> (String, String) {
    let text = std::fs::read_to_string(git_dir.join("BISECT_TERMS")).unwrap_or_default();
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    match (lines.next(), lines.next()) {
        (Some(bad), Some(good)) => (bad.to_string(), good.to_string()),
        _ => ("bad".to_string(), "good".to_string()),
    }
}

/// `BISECT_NAMES`: the paths the bisect was started with, as git's
/// `sq_quote_argv` wrote them (` 'a b' 'it'\''s'`).
fn parse_sq_quoted(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.peek().is_none() {
            return out;
        }
        let mut arg = String::new();
        while let Some(&c) = chars.peek() {
            match c {
                '\'' => {
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\'' {
                            break;
                        }
                        arg.push(c);
                    }
                }
                '\\' => {
                    chars.next();
                    if let Some(c) = chars.next() {
                        arg.push(c);
                    }
                }
                c if c.is_whitespace() => break,
                c => {
                    chars.next();
                    arg.push(c);
                }
            }
        }
        out.push(arg);
    }
}

/// The commits a bisect has left, once a bad and a good commit are known.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BisectRange {
    /// `git rev-list <bad> --not <good>…` with the bisect's paths and
    /// `--first-parent`: every commit that may still be the first bad one,
    /// newest first, the bad commit included.
    pub candidates: Vec<String>,
}

impl BisectRange {
    /// The first bad commit, once it is the only candidate left.
    pub fn first_bad(&self) -> Option<&str> {
        match self.candidates.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}

/// git's `estimate_bisect_steps` (`bisect.c`) over `all` candidates: about
/// how many more commits are tested after the current one, the number in
/// "Bisecting: … (roughly N steps)".
pub fn estimate_steps(all: usize) -> u32 {
    if all < 3 {
        return 0;
    }
    let n = usize::BITS - 1 - all.leading_zeros();
    let e = 1usize << n;
    let x = all - e;
    if e < 3 * x { n } else { n - 1 }
}

/// [`BisectRange`] of `state`; `None` until a bad and a good commit are known.
pub fn bisect_range(
    git: Arc<GitBinary>,
    workdir: &Path,
    state: &BisectState,
) -> Result<Option<BisectRange>> {
    let Some(bad) = state.bad.as_deref() else {
        return Ok(None);
    };
    if state.good.is_empty() {
        return Ok(None);
    }
    let dir = crate::paths::git_dir(workdir);
    let mut cmd = GitCommand::new(git).arg("rev-list");
    if dir.join("BISECT_FIRST_PARENT").exists() {
        cmd = cmd.arg("--first-parent");
    }
    let names = std::fs::read_to_string(dir.join("BISECT_NAMES"))
        .map(|text| parse_sq_quoted(&text))
        .unwrap_or_default();
    let out = cmd
        .arg(bad)
        .arg("--not")
        .args(&state.good)
        .arg("--")
        .args(names)
        .current_dir(workdir)
        .run()?;
    Ok(Some(BisectRange {
        candidates: String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    }))
}

/// `git bisect start`: the current branch (or commit) is where
/// [`bisect_reset`] goes back to. Nothing is checked out until a bad and a
/// good commit are marked.
pub fn bisect_start(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["bisect", "start"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `git bisect <term> [<sha>]`: mark `sha` (HEAD when `None`) with the
/// bisect's own term for `verdict`. Once a bad and a good commit are known
/// git checks out the next commit to test; it exits 2 when only skipped
/// commits are left, which is not a failure here.
pub fn bisect_mark(
    git: Arc<GitBinary>,
    workdir: &Path,
    verdict: BisectVerdict,
    sha: Option<&str>,
) -> Result<()> {
    let (bad, good) = read_terms(&crate::paths::git_dir(workdir));
    let term = match verdict {
        BisectVerdict::Good => good,
        BisectVerdict::Bad => bad,
        BisectVerdict::Skip => "skip".to_string(),
    };
    GitCommand::new(git)
        .args(["bisect", term.as_str()])
        .args(sha)
        .current_dir(workdir)
        .allow_exit_code(2)
        .run()?;
    Ok(())
}

/// `git bisect reset`: end the bisect and check out the branch (or commit)
/// it started from.
pub fn bisect_reset(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["bisect", "reset"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn repo(commits: usize) -> (tempfile::TempDir, Arc<GitBinary>, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}");
            String::from_utf8(out.stdout).unwrap()
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "core.autocrlf", "false"]);
        run(&["config", "commit.gpgsign", "false"]);
        let mut shas = Vec::new();
        for i in 1..=commits {
            std::fs::write(dir.path().join("f.txt"), format!("{i}\n")).unwrap();
            run(&["add", "."]);
            run(&["commit", "-q", "-m", &format!("c{i}")]);
            shas.push(run(&["rev-parse", "HEAD"]).trim().to_string());
        }
        (dir, Arc::new(crate::find_git().unwrap()), shas)
    }

    #[test]
    fn steps_match_git() {
        // "Bisecting: 9 revisions left to test after this (roughly 3 steps)"
        // for 19 candidates, 6 for 100
        assert_eq!(estimate_steps(1), 0);
        assert_eq!(estimate_steps(3), 1);
        assert_eq!(estimate_steps(19), 3);
        assert_eq!(estimate_steps(100), 6);
        assert_eq!(estimate_steps(1024), 9);
    }

    #[test]
    fn parses_quoted_names() {
        assert_eq!(
            parse_sq_quoted(" 'src' 'a b' 'it'\\''s'\n"),
            vec!["src", "a b", "it's"]
        );
        assert!(parse_sq_quoted("\n").is_empty());
    }

    #[test]
    fn reads_refs_with_custom_terms() {
        let state = parse_refs(
            "aaa refs/bisect/fixed\nbbb refs/bisect/broken-bbb\nccc refs/bisect/skip-ccc\n",
            BisectState {
                term_bad: "fixed".into(),
                term_good: "broken".into(),
                ..Default::default()
            },
        );
        assert_eq!(state.bad.as_deref(), Some("aaa"));
        assert_eq!(state.good, vec!["bbb"]);
        assert_eq!(state.skipped, vec!["ccc"]);
    }

    /// 100 commits, the 37th introduced the bug: about 7 tests name it.
    #[test]
    fn finds_the_first_bad_commit() {
        let (dir, git, shas) = repo(100);
        let path = dir.path();
        assert!(!bisect_in_progress(path));
        assert_eq!(bisect_state(git.clone(), path).unwrap(), None);
        bisect_start(git.clone(), path).unwrap();
        bisect_mark(git.clone(), path, BisectVerdict::Bad, None).unwrap();
        let state = bisect_state(git.clone(), path).unwrap().unwrap();
        assert_eq!(state.start, "main");
        assert_eq!(state.start_branch(), Some("main"));
        assert_eq!(state.bad.as_deref(), Some(shas[99].as_str()));
        assert_eq!(bisect_range(git.clone(), path, &state).unwrap(), None);
        bisect_mark(git.clone(), path, BisectVerdict::Good, Some(&shas[0])).unwrap();
        let culprit = 36;
        let mut tests = 0;
        loop {
            let state = bisect_state(git.clone(), path).unwrap().unwrap();
            let range = bisect_range(git.clone(), path, &state).unwrap().unwrap();
            if let Some(first) = range.first_bad() {
                assert_eq!(first, shas[culprit]);
                break;
            }
            if tests == 0 {
                assert_eq!(range.candidates.len(), 99);
                assert_eq!(estimate_steps(range.candidates.len()) + 1, 7);
            }
            let head = crate::head_sha(git.clone(), path).unwrap();
            let ix = shas.iter().position(|s| *s == head).unwrap();
            let verdict = if ix >= culprit {
                BisectVerdict::Bad
            } else {
                BisectVerdict::Good
            };
            bisect_mark(git.clone(), path, verdict, None).unwrap();
            tests += 1;
            assert!(tests <= 8);
        }
        assert!((6..=8).contains(&tests), "{tests} tests");
        bisect_reset(git.clone(), path).unwrap();
        assert!(!bisect_in_progress(path));
        assert_eq!(crate::head_sha(git, path).unwrap(), shas[99]);
    }

    #[test]
    fn only_skipped_left_is_not_an_error() {
        let (dir, git, shas) = repo(4);
        let path = dir.path();
        bisect_start(git.clone(), path).unwrap();
        bisect_mark(git.clone(), path, BisectVerdict::Bad, Some(&shas[3])).unwrap();
        bisect_mark(git.clone(), path, BisectVerdict::Good, Some(&shas[0])).unwrap();
        bisect_mark(git.clone(), path, BisectVerdict::Skip, None).unwrap();
        bisect_mark(git.clone(), path, BisectVerdict::Skip, None).unwrap();
        let state = bisect_state(git.clone(), path).unwrap().unwrap();
        assert_eq!(state.skipped.len(), 2);
        let range = bisect_range(git.clone(), path, &state).unwrap().unwrap();
        assert_eq!(range.candidates.len(), 3);
        assert_eq!(range.first_bad(), None);
        bisect_reset(git, path).unwrap();
    }
}
