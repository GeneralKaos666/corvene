//! Corvene `798-blame`: the Blame view's state and actions. GitHub Desktop
//! has no blame.
//!
//! [`Dispatcher::show_blame`] reads the file at the revision (or in the
//! working tree) and shows its lines at once, then streams
//! `git blame --incremental` on a background thread
//! ([`corvene_git::blame`]); the result so far reaches the view every
//! [`BLAME_PUBLISH_INTERVAL`]. Closing the view, opening another blame or
//! selecting another file or commit stops git. Finished results are kept
//! in [`crate::diff_cache`] by commit (the working tree: `HEAD` plus a
//! hash of the file), path and options.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use corvene_git::{BlameCommit, BlameEvent, BlameOptions, CancelToken, UNCOMMITTED_SHA};
use corvene_models::{FileStatusKind, Section};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};

/// How often a running blame hands its lines to the view.
pub const BLAME_PUBLISH_INTERVAL: Duration = Duration::from_millis(60);

/// Files above this size are not blamed.
pub const BLAME_MAX_BYTES: usize = 10 * 1024 * 1024;

/// What a Blame view shows: `path` at `rev`, or in the working tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlameTarget {
    pub path: String,
    /// `None`: the working tree, uncommitted lines included.
    pub rev: Option<String>,
    /// The line (0-based) to scroll to once the file is shown.
    pub line: Option<u32>,
}

/// Where one line comes from: an index into [`BlameResult::commits`], its
/// line number there (1-based) and an index into [`BlameResult::files`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineBlame {
    pub commit: u32,
    pub orig_line: u32,
    pub file: u32,
}

#[derive(Clone, Debug, Default)]
pub struct BlameResult {
    pub commits: Vec<BlameCommit>,
    /// The file's paths in the commits (a rename gives more than one).
    pub files: Vec<String>,
    /// One per line of the file; `None` while git has not reached it.
    pub lines: Vec<Option<LineBlame>>,
}

impl BlameResult {
    fn new(lines: usize) -> Self {
        Self {
            lines: vec![None; lines],
            ..Self::default()
        }
    }

    pub fn commit_of(&self, line: usize) -> Option<&BlameCommit> {
        let blame = self.lines.get(line).copied().flatten()?;
        self.commits.get(blame.commit as usize)
    }

    pub fn file_of(&self, line: usize) -> Option<&str> {
        let blame = self.lines.get(line).copied().flatten()?;
        self.files.get(blame.file as usize).map(String::as_str)
    }

    /// Lines git has attributed so far.
    pub fn attributed(&self) -> usize {
        self.lines.iter().filter(|l| l.is_some()).count()
    }

    fn size(&self) -> usize {
        self.lines.len() * std::mem::size_of::<Option<LineBlame>>()
            + self
                .commits
                .iter()
                .map(|c| c.summary.len() + c.author_name.len() + c.author_email.len() + 160)
                .sum::<usize>()
            + self.files.iter().map(|f| f.len() + 24).sum::<usize>()
    }
}

/// Builds a [`BlameResult`] from the events of [`corvene_git::blame`].
struct Builder {
    result: BlameResult,
    commits: HashMap<String, u32>,
    files: HashMap<String, u32>,
}

impl Builder {
    fn new(lines: usize) -> Self {
        Self {
            result: BlameResult::new(lines),
            commits: HashMap::new(),
            files: HashMap::new(),
        }
    }

    fn apply(&mut self, event: BlameEvent) {
        match event {
            BlameEvent::Commit(commit) => {
                let ix = self.result.commits.len() as u32;
                self.commits.insert(commit.sha.clone(), ix);
                self.result.commits.push(commit);
            }
            BlameEvent::Range(range) => {
                let Some(&commit) = self.commits.get(&range.sha) else {
                    return;
                };
                let file = match self.files.get(&range.filename) {
                    Some(&ix) => ix,
                    None => {
                        let ix = self.result.files.len() as u32;
                        self.files.insert(range.filename.clone(), ix);
                        self.result.files.push(range.filename);
                        ix
                    }
                };
                for i in 0..range.count {
                    let ix = (range.final_line + i) as usize;
                    if let Some(slot) = ix
                        .checked_sub(1)
                        .and_then(|ix| self.result.lines.get_mut(ix))
                    {
                        *slot = Some(LineBlame {
                            commit,
                            orig_line: range.orig_line + i,
                            file,
                        });
                    }
                }
            }
        }
    }

    /// Lines git never attributed (a file `HEAD` does not have) are not
    /// committed yet.
    fn finish(mut self, path: &str) -> BlameResult {
        if self.result.lines.iter().any(Option::is_none) {
            let commit = match self.commits.get(UNCOMMITTED_SHA) {
                Some(&ix) => ix,
                None => {
                    self.result.commits.push(BlameCommit {
                        sha: UNCOMMITTED_SHA.to_string(),
                        author_name: "Not Committed Yet".to_string(),
                        summary: "Not committed yet".to_string(),
                        ..BlameCommit::default()
                    });
                    (self.result.commits.len() - 1) as u32
                }
            };
            let file = self.result.files.len() as u32;
            self.result.files.push(path.to_string());
            for (ix, slot) in self.result.lines.iter_mut().enumerate() {
                if slot.is_none() {
                    *slot = Some(LineBlame {
                        commit,
                        orig_line: ix as u32 + 1,
                        file,
                    });
                }
            }
        }
        self.result
    }
}

/// A repository's open Blame view.
#[derive(Clone, Debug)]
pub struct BlameState {
    pub target: BlameTarget,
    /// The tab the view replaces the diff of; leaving it closes the view.
    pub section: Section,
    /// The commit `target.rev` names, once read.
    pub commit: Option<String>,
    /// The file's lines; `None` while they load.
    pub contents: Option<Arc<Vec<String>>>,
    pub result: Arc<BlameResult>,
    /// git is still attributing lines.
    pub loading: bool,
    /// Why there is nothing to show (binary, too large, git failed).
    pub message: Option<String>,
    /// Bumped whenever the view has something new to draw.
    pub generation: u64,
    /// What "Back" returns to, most recent last.
    pub back: Vec<BlameTarget>,
    /// Identifies this load; results of an older one are dropped.
    load: u64,
    cancel: CancelToken,
}

impl BlameState {
    /// Changes with every [`Dispatcher::show_blame`].
    pub fn load_id(&self) -> u64 {
        self.load
    }

    /// Whether `ix` starts a run of lines from one commit.
    pub fn starts_group(&self, ix: usize) -> bool {
        let line = |ix: usize| {
            self.result
                .lines
                .get(ix)
                .copied()
                .flatten()
                .map(|l| l.commit)
        };
        ix == 0 || line(ix) != line(ix - 1) || line(ix).is_none()
    }
}

/// What the background load sends to the main thread.
enum BlameMessage {
    Contents {
        commit: Option<String>,
        lines: Arc<Vec<String>>,
    },
    Progress(Arc<BlameResult>),
    Done(Arc<BlameResult>),
    Failed(String),
}

static NEXT_LOAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Dispatcher {
    /// "Blame" on a changed file: the working tree, or `HEAD` for a
    /// deleted file.
    pub fn show_blame_for_change(id: u64, path: String, cx: &mut dyn Host) {
        let deleted = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.status.as_deref())
            .and_then(|s| s.files.iter().find(|f| f.path == path))
            .is_some_and(|f| f.status.kind == FileStatusKind::Deleted);
        let rev = deleted.then(|| "HEAD".to_string());
        Self::show_blame(
            id,
            BlameTarget {
                path,
                rev,
                line: None,
            },
            Vec::new(),
            cx,
        );
    }

    /// "Blame" on a file of the selected commit (the newest of a range):
    /// the file as that commit left it (as its parent had it, for a deleted file).
    pub fn show_blame_for_commit_file(id: u64, path: String, cx: &mut dyn Host) {
        let target = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            // the newest commit of a selected range, as its diff shows
            let Some(sha) = Self::ordered_selection(rs)
                .pop()
                .or_else(|| rs.selected_commit.clone())
            else {
                return;
            };
            let file = rs
                .changeset
                .as_ref()
                .and_then(|c| c.files.iter().find(|f| f.path == path));
            match file {
                Some(f) if f.status.kind == FileStatusKind::Deleted => BlameTarget {
                    path,
                    rev: Some(format!("{sha}^")),
                    line: None,
                },
                _ => BlameTarget {
                    path,
                    rev: Some(sha),
                    line: None,
                },
            }
        };
        Self::show_blame(id, target, Vec::new(), cx);
    }

    /// Open the Blame view on `target`; `back` is what "Back" returns to.
    pub fn show_blame(id: u64, target: BlameTarget, back: Vec<BlameTarget>, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let load = NEXT_LOAD.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cancel = CancelToken::new();
        let options = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if let Some(old) = rs.blame.take() {
                old.cancel.cancel();
            }
            rs.blame = Some(BlameState {
                target: target.clone(),
                section: rs.section,
                commit: None,
                contents: None,
                result: Arc::new(BlameResult::default()),
                loading: true,
                message: None,
                generation: load,
                back,
                load,
                cancel: cancel.clone(),
            });
            cx.notify();
            rs.blame_options
        });

        let (tx, rx) = async_channel::unbounded::<BlameMessage>();
        let task = cx.background_executor().spawn(async move {
            let send = |m: BlameMessage| {
                let _ = tx.send_blocking(m);
            };
            if let Err(message) = run_blame(git, &workdir, &target, options, &cancel, &send)
                && !cancel.is_cancelled()
            {
                send(BlameMessage::Failed(message));
            }
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            while let Ok(first) = rx.recv().await {
                // everything that queued up while the main thread was busy
                // is applied in one go
                let mut batch = vec![first];
                while let Ok(more) = rx.try_recv() {
                    batch.push(more);
                }
                cx.update(|cx| Self::apply_blame(id, load, batch, cx));
            }
            task.await;
        })
        .detach();
    }

    fn apply_blame(id: u64, load: u64, batch: Vec<BlameMessage>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            let Some(blame) = s.repo_state_mut(id).blame.as_mut() else {
                return;
            };
            if blame.load != load {
                return;
            }
            for message in batch {
                match message {
                    BlameMessage::Contents { commit, lines } => {
                        blame.commit = commit;
                        blame.contents = Some(lines);
                    }
                    BlameMessage::Progress(result) => blame.result = result,
                    BlameMessage::Done(result) => {
                        blame.result = result;
                        blame.loading = false;
                    }
                    BlameMessage::Failed(message) => {
                        blame.message = Some(message);
                        blame.loading = false;
                    }
                }
            }
            blame.generation += 1;
            cx.notify();
        });
    }

    /// Close the Blame view and stop git.
    pub fn close_blame(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(rs) = s.repo_states.get_mut(&id)
                && let Some(blame) = rs.blame.take()
            {
                blame.cancel.cancel();
                cx.notify();
            }
        });
    }

    /// Close a Blame view that does not belong to what is now shown: the
    /// selection or the tab changed.
    pub(crate) fn close_blame_unless(
        id: u64,
        keep: impl Fn(&BlameState) -> bool,
        cx: &mut dyn Host,
    ) {
        let open = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.blame.as_ref())
            .is_some_and(|b| !keep(b));
        if open {
            Self::close_blame(id, cx);
        }
    }

    /// "Blame previous revision": blame the file as it was before the
    /// commit that last changed line `line`, scrolled to that line.
    /// `top_line` is where Back returns to.
    pub fn blame_previous_revision(id: u64, line: usize, top_line: u32, cx: &mut dyn Host) {
        let next = {
            let s = Self::state(cx).read(cx);
            let Some(blame) = s.repo_states.get(&id).and_then(|rs| rs.blame.as_ref()) else {
                return;
            };
            let Some(commit) = blame.result.commit_of(line) else {
                return;
            };
            let orig_line = blame.result.lines[line].map_or(1, |l| l.orig_line);
            let Some((sha, path)) = commit.previous.clone() else {
                return;
            };
            let mut back = blame.back.clone();
            back.push(BlameTarget {
                line: Some(top_line),
                ..blame.target.clone()
            });
            (
                BlameTarget {
                    path,
                    rev: Some(sha),
                    line: Some(orig_line.saturating_sub(1)),
                },
                back,
            )
        };
        Self::show_blame(id, next.0, next.1, cx);
    }

    /// Back to the blame "Blame previous revision" came from.
    pub fn blame_back(id: u64, cx: &mut dyn Host) {
        let previous = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| {
                let blame = rs.blame.as_ref()?;
                let mut back = blame.back.clone();
                let target = back.pop()?;
                Some((target, back))
            });
        if let Some((target, back)) = previous {
            Self::show_blame(id, target, back, cx);
        }
    }

    /// Change the blame options and blame the shown file again.
    pub fn set_blame_options(id: u64, options: BlameOptions, cx: &mut dyn Host) {
        let reload = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            rs.blame_options = options;
            rs.blame
                .as_ref()
                .map(|b| (b.target.clone(), b.back.clone()))
        });
        if let Some((target, back)) = reload {
            Self::show_blame(id, target, back, cx);
        }
    }

    /// A gutter cell was clicked: History with that commit selected
    /// (Changes with the file selected, for uncommitted lines).
    pub fn show_blamed_commit(id: u64, sha: String, cx: &mut dyn Host) {
        let path = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.blame.as_ref())
            .map(|b| b.target.path.clone());
        Self::close_blame(id, cx);
        if sha == UNCOMMITTED_SHA {
            Self::show_section(id, Section::Changes, cx);
            if let Some(path) = path {
                Self::select_file(id, path, cx);
            }
            return;
        }
        Self::reveal_commit(id, sha, cx);
    }

    /// Select `sha` in History, loading as many pages of the list as it
    /// takes to reach it. A commit `HEAD` does not reach is searched for
    /// when `886-history-search` is on.
    pub fn reveal_commit(id: u64, sha: String, cx: &mut dyn Host) {
        Self::show_section(id, Section::History, cx);
        let visible = |cx: &mut dyn Host| {
            Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .is_some_and(|rs| rs.visible_commits().iter().any(|c| c.sha == sha))
        };
        if !visible(cx) {
            Self::exit_compare(id, cx);
            Self::clear_history_filter(id, cx);
        }
        if visible(cx) {
            Self::select_and_reveal(id, sha, cx);
            return;
        }
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (first_parent, loaded, search) = {
            let s = Self::state(cx).read(cx);
            (
                Self::history_first_parent(s),
                s.repo_states.get(&id).map_or(0, |rs| rs.commits.len()),
                s.flags.bool(crate::flags::ids::HISTORY_SEARCH),
            )
        };
        let wanted = sha.clone();
        crate::remote::spawn_bg(
            cx,
            move || {
                let mut limit = (loaded * 4).max(1000);
                loop {
                    let commits =
                        corvene_git::get_commits_with(&workdir, "HEAD", 0, limit, first_parent)
                            .ok()?;
                    if let Some(ix) = commits.iter().position(|c| c.sha == wanted) {
                        // whole pages, so scrolling on loads the next one
                        let batch = corvene_git::COMMIT_BATCH_SIZE;
                        let keep = (ix / batch + 1) * batch;
                        let exhausted = commits.len() <= keep && commits.len() < limit;
                        return Some((
                            commits.into_iter().take(keep).collect::<Vec<_>>(),
                            exhausted,
                        ));
                    }
                    if commits.len() < limit || limit >= 1_000_000 {
                        return None;
                    }
                    limit *= 4;
                }
            },
            move |found, cx| match found {
                Some((commits, exhausted)) => {
                    Self::state(cx).update(cx, |s, cx| {
                        let rs = s.repo_state_mut(id);
                        if rs.commits.len() < commits.len() {
                            rs.commits = commits;
                            rs.commits_exhausted = exhausted;
                            cx.notify();
                        }
                    });
                    Self::select_and_reveal(id, sha, cx);
                }
                None if search => Self::set_history_filter_text(id, sha, cx),
                None => warn!(id, %sha, "blamed commit is not in History"),
            },
        );
    }

    fn select_and_reveal(id: u64, sha: String, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.reveal_commit = (rs.reveal_commit.0 + 1, Some(sha.clone()));
            cx.notify();
        });
        Self::select_commit(id, sha, cx);
    }
}

/// Read the file and blame it, sending what is ready to `send`.
fn run_blame(
    git: Arc<corvene_git::GitBinary>,
    workdir: &Path,
    target: &BlameTarget,
    options: BlameOptions,
    cancel: &CancelToken,
    send: &dyn Fn(BlameMessage),
) -> Result<(), String> {
    let path = target.path.as_str();
    let (bytes, commit, key) = match &target.rev {
        Some(rev) => {
            let commit = corvene_git::resolve_commit(workdir, rev)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("{rev} is not a commit"))?;
            let bytes = corvene_git::blob_bytes(git.clone(), workdir, &commit, path)
                .map_err(|_| format!("{path} is not in commit {}", &commit[..7]))?;
            let key = commit.clone();
            (bytes, Some(commit), key)
        }
        None => {
            let bytes = std::fs::read(workdir.join(path)).map_err(|e| e.to_string())?;
            let head = corvene_git::resolve_commit(workdir, "HEAD")
                .ok()
                .flatten()
                .unwrap_or_default();
            let key = format!("worktree:{head}:{:016x}", hash(&bytes));
            (bytes, None, key)
        }
    };
    if bytes.len() > BLAME_MAX_BYTES {
        return Err("This file is too large to blame.".to_string());
    }
    if bytes[..bytes.len().min(8000)].contains(&0) {
        return Err("Binary files cannot be blamed.".to_string());
    }
    let lines = Arc::new(corvene_git::file_lines(&bytes));
    let line_count = lines.len();
    send(BlameMessage::Contents {
        commit: commit.clone(),
        lines,
    });
    if let Some(result) = crate::diff_cache::blame(workdir, &key, path, options) {
        send(BlameMessage::Done(result));
        return Ok(());
    }

    let mut builder = Builder::new(line_count);
    let mut published = Instant::now();
    corvene_git::blame(
        git,
        workdir,
        commit.as_deref(),
        path,
        options,
        Some(cancel.clone()),
        |event| {
            builder.apply(event);
            if published.elapsed() >= BLAME_PUBLISH_INTERVAL {
                published = Instant::now();
                send(BlameMessage::Progress(Arc::new(builder.result.clone())));
            }
        },
    )
    .map_err(|e| e.to_string())?;
    let result = Arc::new(builder.finish(path));
    crate::diff_cache::store_blame(workdir, &key, path, options, result.clone(), result.size());
    send(BlameMessage::Done(result));
    Ok(())
}

fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_git::BlameRange;

    fn commit(sha: &str) -> BlameEvent {
        BlameEvent::Commit(BlameCommit {
            sha: sha.to_string(),
            ..BlameCommit::default()
        })
    }

    fn range(sha: &str, final_line: u32, count: u32) -> BlameEvent {
        BlameEvent::Range(BlameRange {
            sha: sha.to_string(),
            orig_line: final_line + 10,
            final_line,
            count,
            filename: "a.txt".to_string(),
        })
    }

    #[test]
    fn builds_lines_and_marks_the_rest_uncommitted() {
        let mut builder = Builder::new(4);
        builder.apply(commit("a"));
        builder.apply(range("a", 2, 2));
        // past the end of the file: ignored
        builder.apply(range("a", 9, 1));
        assert_eq!(builder.result.attributed(), 2);
        let result = builder.finish("a.txt");
        assert_eq!(result.lines[1].unwrap().orig_line, 12);
        assert_eq!(result.lines[2].unwrap().commit, 0);
        assert!(result.commit_of(0).unwrap().is_uncommitted());
        assert!(result.commit_of(3).unwrap().is_uncommitted());
        assert_eq!(result.attributed(), 4);
    }
}
