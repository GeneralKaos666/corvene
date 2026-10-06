//! Corvene (`1307-commit-progress`): how far a commit got, for the commit
//! button ("Committing 1,234 of 5,000 files", then "Writing commit to
//! main", with a thin bar). GHD (`app/src/ui/changes/commit-message.tsx`,
//! `getButtonTitle`) says "Committing 5000 files to main" until git is done,
//! which looks stuck on a big commit (desktop/desktop#19679).
//!
//! The thread that commits reports through a [`CommitProgressReporter`] (a
//! count at most every [`REPORT_INTERVAL`]); the repository's
//! `commit_progress` is only set once the commit has run for
//! [`crate::state::BUSY_INDICATOR_DELAY`], so a quick commit keeps GHD's
//! label and nothing flickers.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};

/// What the commit is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPhase {
    /// `update-index` and `apply --cached` put the files in the index.
    Staging,
    /// `git commit` writes the tree and the commit (and runs the hooks).
    Writing,
}

/// The commit in progress: its phase and how many of its files are staged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitProgress {
    pub phase: CommitPhase,
    /// Files in the index so far.
    pub staged: usize,
    /// Files going into the commit.
    pub total: usize,
}

impl CommitProgress {
    /// How full the bar is, 0 to 1; full while git writes the commit.
    pub fn fraction(&self) -> f32 {
        match self.phase {
            CommitPhase::Writing => 1.,
            CommitPhase::Staging if self.total == 0 => 0.,
            CommitPhase::Staging => self.staged.min(self.total) as f32 / self.total as f32,
        }
    }
}

/// The least time between two counts the git thread sends.
pub const REPORT_INTERVAL: Duration = Duration::from_millis(50);

/// The git thread's side: counts and the switch to writing, sent to the
/// foreground at most every [`REPORT_INTERVAL`] (a new phase and the last
/// file go at once).
pub struct CommitProgressReporter {
    tx: async_channel::Sender<CommitProgress>,
    total: usize,
    last: Option<(Instant, CommitProgress)>,
}

impl CommitProgressReporter {
    /// `staged` of the files are in the index.
    pub fn staged(&mut self, staged: usize) {
        self.report(CommitProgress {
            phase: CommitPhase::Staging,
            staged: staged.min(self.total),
            total: self.total,
        });
    }

    /// Every file is staged; `git commit` runs.
    pub fn writing(&mut self) {
        self.report(CommitProgress {
            phase: CommitPhase::Writing,
            staged: self.total,
            total: self.total,
        });
    }

    fn report(&mut self, progress: CommitProgress) {
        let now = Instant::now();
        if !worth_sending(self.last.as_ref(), now, &progress) {
            return;
        }
        self.last = Some((now, progress));
        tracing::debug!(
            phase = ?progress.phase,
            staged = progress.staged,
            total = progress.total,
            "commit progress"
        );
        let _ = self.tx.send_blocking(progress);
    }
}

fn worth_sending(
    last: Option<&(Instant, CommitProgress)>,
    now: Instant,
    next: &CommitProgress,
) -> bool {
    let Some((at, previous)) = last else {
        return true;
    };
    previous != next
        && (previous.phase != next.phase
            || next.staged == next.total
            || now.duration_since(*at) >= REPORT_INTERVAL)
}

/// What the foreground knows of the commit's progress.
#[derive(Default)]
struct Seen {
    latest: Option<CommitProgress>,
    /// The commit has run long enough for the button to say.
    shown: bool,
    /// The thread that commits is done.
    done: bool,
}

/// Follows the commit of `total` files in the repository `id`: the
/// reporter goes to the thread that commits, its progress into
/// `RepositoryState::commit_progress` once the commit has run for
/// [`crate::state::BUSY_INDICATOR_DELAY`].
pub fn commit_progress_ui(id: u64, total: usize, cx: &mut dyn Host) -> CommitProgressReporter {
    let (tx, rx) = async_channel::unbounded::<CommitProgress>();
    let seen = Rc::new(RefCell::new(Seen::default()));
    cx.spawn({
        let seen = seen.clone();
        async move |cx: &mut AsyncCtx| {
            cx.background_executor()
                .timer(crate::state::BUSY_INDICATOR_DELAY)
                .await;
            let latest = {
                let mut seen = seen.borrow_mut();
                if seen.done {
                    return;
                }
                seen.shown = true;
                seen.latest
            };
            if let Some(progress) = latest {
                cx.update(|cx| show(id, progress, cx));
            }
        }
    })
    .detach();
    // ends once the commit dropped its reporter
    cx.spawn(async move |cx: &mut AsyncCtx| {
        while let Ok(mut progress) = rx.recv().await {
            while let Ok(newer) = rx.try_recv() {
                progress = newer;
            }
            let shown = {
                let mut seen = seen.borrow_mut();
                seen.latest = Some(progress);
                seen.shown
            };
            if shown {
                cx.update(|cx| show(id, progress, cx));
            }
        }
        seen.borrow_mut().done = true;
    })
    .detach();
    CommitProgressReporter {
        tx,
        total,
        last: None,
    }
}

/// Only while the commit runs: a count that arrives after it ended (or the
/// timer firing then) changes nothing.
fn show(id: u64, progress: CommitProgress, cx: &mut dyn Host) {
    Dispatcher::state(cx).update(cx, |s, cx| {
        if let Some(rs) = s.repo_states.get_mut(&id)
            && rs.committing
            && rs.commit_progress != Some(progress)
        {
            rs.commit_progress = Some(progress);
            cx.notify();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staging(staged: usize) -> CommitProgress {
        CommitProgress {
            phase: CommitPhase::Staging,
            staged,
            total: 10,
        }
    }

    #[test]
    fn counts_are_sent_at_most_every_interval() {
        let start = Instant::now();
        let last = (start, staging(1));
        let soon = start + Duration::from_millis(10);
        let later = start + REPORT_INTERVAL;
        assert!(worth_sending(None, start, &staging(0)));
        assert!(!worth_sending(Some(&last), soon, &staging(2)));
        assert!(worth_sending(Some(&last), later, &staging(2)));
        // the same count again is never sent
        assert!(!worth_sending(Some(&last), later, &staging(1)));
        // the last file and a new phase go at once
        assert!(worth_sending(Some(&last), soon, &staging(10)));
        let writing = CommitProgress {
            phase: CommitPhase::Writing,
            staged: 10,
            total: 10,
        };
        assert!(worth_sending(Some(&(start, staging(10))), soon, &writing));
    }

    #[test]
    fn the_bar_fills_with_the_staged_files() {
        assert_eq!(staging(0).fraction(), 0.);
        assert_eq!(staging(5).fraction(), 0.5);
        assert_eq!(staging(12).fraction(), 1.);
        let empty = CommitProgress {
            phase: CommitPhase::Staging,
            staged: 0,
            total: 0,
        };
        assert_eq!(empty.fraction(), 0.);
        let writing = CommitProgress {
            phase: CommitPhase::Writing,
            ..empty
        };
        assert_eq!(writing.fraction(), 1.);
    }
}
