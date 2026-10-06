//! Corvene `1216-recent-activity`: Repository › Recent Activity…, the
//! reflog as a History mode. GitHub Desktop has none; the reflog is only
//! reachable on the command line there.
//!
//! [`Dispatcher::show_recent_activity`] swaps History's commit list for the
//! reflog of HEAD (or the current branch) read with gitoxide
//! ([`corvene_git::read_reflog`]). While it is open
//! [`RepositoryState::visible_commits`] lists the commits the entries point
//! at, so selecting an entry fills the usual commit view, also for commits
//! no branch reaches any more. Those are marked
//! ([`corvene_git::reachable_shas`]), and a branch the HEAD log switched
//! away from that no longer exists can be restored at the tip it had then.
//! The list reloads with History on every refresh (the watcher sees
//! `logs/`). Closing restores History's selection.

use std::collections::{HashMap, HashSet};

use corvene_git::{RebaseStep, ReflogAction, ReflogEntry};
use corvene_models::{Commit, Section, Tip};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::state::{AppState, Popup, RepositoryState};

/// The most reflog lines read (git keeps 90 days of them by default).
pub const REFLOG_LIMIT: usize = 2000;

/// Whose reflog the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReflogSource {
    #[default]
    Head,
    /// The checked-out branch's own log (`refs/heads/<name>`).
    CurrentBranch,
}

/// One row of the list: an entry, and for a collapsed rebase the number of
/// step entries folded into it (start and picks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReflogRow {
    pub entry: usize,
    pub folded: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ReflogState {
    pub source: ReflogSource,
    /// The branch [`ReflogSource::CurrentBranch`] read, for the header.
    pub branch: Option<String>,
    /// Newest first.
    pub entries: Vec<ReflogEntry>,
    /// What the list shows ([`build_rows`]).
    pub rows: Vec<ReflogRow>,
    /// Each existing commit an entry points at, once, in entry order.
    pub commits: Vec<Commit>,
    /// Entry targets no branch, tag or HEAD reaches.
    pub unreachable: HashSet<String>,
    /// Entry index → a branch that log switched away from and that is gone;
    /// the entry's `old` is the tip it had then.
    pub deleted_branches: HashMap<usize, String>,
    /// "Show every step": rebases are not folded.
    pub show_steps: bool,
    /// "Only unreachable".
    pub only_unreachable: bool,
    /// The selected entry (index into `entries`).
    pub selected: Option<usize>,
    pub loading: bool,
    pub loaded: bool,
    pub reload_pending: bool,
    /// History's selection when the list opened, restored on close.
    pub saved_selection: Vec<String>,
}

impl ReflogState {
    /// Whether `entry` points at a commit that still exists.
    pub fn has_commit(&self, entry: usize) -> bool {
        self.entries
            .get(entry)
            .is_some_and(|e| self.commits.iter().any(|c| c.sha == e.new))
    }

    pub fn commit(&self, entry: usize) -> Option<&Commit> {
        let sha = &self.entries.get(entry)?.new;
        self.commits.iter().find(|c| &c.sha == sha)
    }

    pub fn is_unreachable(&self, entry: usize) -> bool {
        self.entries
            .get(entry)
            .is_some_and(|e| self.unreachable.contains(&e.new))
    }

    /// The row showing `entry`.
    pub fn row_of(&self, entry: usize) -> Option<usize> {
        self.rows.iter().position(|r| r.entry == entry)
    }

    fn rebuild_rows(&mut self) {
        self.rows = build_rows(
            &self.entries,
            self.show_steps,
            self.only_unreachable.then_some(&self.unreachable),
        );
    }
}

/// The list's rows: each entry, a finished or aborted rebase's start and
/// step entries folded into it unless `show_steps`, and only entries whose
/// commit is in `unreachable` when given.
pub fn build_rows(
    entries: &[ReflogEntry],
    show_steps: bool,
    unreachable: Option<&HashSet<String>>,
) -> Vec<ReflogRow> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let mut folded = 0;
        if !show_steps
            && let ReflogAction::Rebase {
                step: RebaseStep::Finish | RebaseStep::Abort,
                ..
            } = entries[i].action
        {
            // newest first: the picks, then the start
            while let Some(ReflogAction::Rebase { step, .. }) =
                entries.get(i + 1 + folded).map(|e| &e.action)
            {
                match step {
                    RebaseStep::Pick => folded += 1,
                    RebaseStep::Start => {
                        folded += 1;
                        break;
                    }
                    _ => break,
                }
            }
        }
        if unreachable.is_none_or(|u| u.contains(&entries[i].new)) {
            rows.push(ReflogRow { entry: i, folded });
        }
        i += 1 + folded;
    }
    rows
}

/// Entries that switched away from a branch which no longer exists (not a
/// local or remote branch now), newest first, each name once.
pub fn deleted_branches(
    entries: &[ReflogEntry],
    existing: &HashSet<String>,
) -> HashMap<usize, String> {
    let mut seen = HashSet::new();
    let mut out = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let ReflogAction::Checkout { from, to } = &e.action else {
            continue;
        };
        // a detached HEAD is named by its full SHA
        let is_sha = from.len() >= 40 && from.bytes().all(|b| b.is_ascii_hexdigit());
        if is_sha || from == to || e.old.is_empty() || existing.contains(from) {
            continue;
        }
        if seen.insert(from.clone()) {
            out.insert(i, from.clone());
        }
    }
    out
}

/// The open list of the selected repository, `None` while the flag is off.
pub fn recent_activity_of<'a>(s: &AppState, rs: &'a RepositoryState) -> Option<&'a ReflogState> {
    if !s.flags.bool(crate::flags::ids::RECENT_ACTIVITY) {
        return None;
    }
    rs.reflog.as_ref()
}

/// Same entry after a reload (new lines are added on top).
fn same_entry(a: &ReflogEntry, b: &ReflogEntry) -> bool {
    a.new == b.new && a.old == b.old && a.seconds == b.seconds && a.message == b.message
}

struct Loaded {
    entries: Vec<ReflogEntry>,
    branch: Option<String>,
    commits: Vec<Commit>,
    unreachable: HashSet<String>,
    deleted: HashMap<usize, String>,
}

impl Dispatcher {
    /// Repository › Recent Activity…: History lists the reflog.
    pub fn show_recent_activity(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::RECENT_ACTIVITY)
        {
            return;
        }
        Self::show_section(id, Section::History, cx);
        let opened = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.reflog.is_some() {
                return false;
            }
            rs.reflog = Some(ReflogState {
                saved_selection: rs.selected_commits.clone(),
                ..Default::default()
            });
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            true
        });
        if opened {
            Self::close_blame(id, cx);
            Self::close_issues(id, cx);
            Self::close_releases(id, cx);
            Self::close_tags(id, cx);
            Self::load_reflog(id, cx);
        }
    }

    /// The header's close button and Escape: back to History, its
    /// selection as it was.
    pub fn close_recent_activity(id: u64, cx: &mut dyn Host) {
        let saved = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_states.get_mut(&id)?;
            let reflog = rs.reflog.take()?;
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            Some(reflog.saved_selection)
        });
        if let Some(saved) = saved
            && !saved.is_empty()
        {
            Self::select_commits(id, saved, cx);
        }
    }

    /// Read the list again (opened, refreshed, its source switched). One
    /// load at a time; one asked for meanwhile runs after it.
    pub fn load_reflog(id: u64, cx: &mut dyn Host) {
        let state = Self::state(cx);
        let job = {
            let s = state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let (Some(reflog), Some(info)) = (rs.reflog.as_ref(), rs.info.as_ref()) else {
                return;
            };
            if reflog.loading {
                None
            } else {
                let branch = match (&reflog.source, &info.tip) {
                    (ReflogSource::CurrentBranch, Tip::Valid { branch }) => {
                        Some(branch.name.clone())
                    }
                    _ => None,
                };
                let existing: HashSet<String> =
                    info.branches.iter().map(|b| b.name.clone()).collect();
                Some((info.workdir.clone(), branch, existing))
            }
        };
        let Some((workdir, branch, existing)) = job else {
            state.update(cx, |s, _| {
                if let Some(r) = s.repo_state_mut(id).reflog.as_mut() {
                    r.reload_pending = true;
                }
            });
            return;
        };
        state.update(cx, |s, _| {
            if let Some(r) = s.repo_state_mut(id).reflog.as_mut() {
                r.loading = true;
            }
        });
        let task = cx.background_executor().spawn(async move {
            let refname = match &branch {
                Some(name) => format!("refs/heads/{name}"),
                None => "HEAD".to_string(),
            };
            let entries = corvene_git::read_reflog(&workdir, &refname, REFLOG_LIMIT)?;
            let shas: Vec<String> = entries.iter().map(|e| e.new.clone()).collect();
            let commits = corvene_git::commits_by_sha(&workdir, &shas)?;
            let present: Vec<String> = commits.iter().map(|c| c.sha.clone()).collect();
            let reachable = corvene_git::reachable_shas(&workdir, &present)?;
            let unreachable = present
                .into_iter()
                .filter(|sha| !reachable.contains(sha))
                .collect();
            let deleted = deleted_branches(&entries, &existing);
            corvene_git::error::Result::Ok(Loaded {
                entries,
                branch,
                commits,
                unreachable,
                deleted,
            })
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let result = task.await;
            cx.update(|cx| {
                let (reload, select) = Self::state(cx).update(cx, |s, cx| {
                    // closed meanwhile
                    let Some(r) = s.repo_state_mut(id).reflog.as_mut() else {
                        return (false, None);
                    };
                    r.loading = false;
                    let reload = std::mem::take(&mut r.reload_pending);
                    let loaded = match result {
                        Ok(loaded) => loaded,
                        Err(err) => {
                            warn!(id, %err, "reflog failed");
                            r.loaded = true;
                            cx.notify();
                            return (reload, None);
                        }
                    };
                    let unchanged = r.loaded
                        && r.branch == loaded.branch
                        && r.entries == loaded.entries
                        && r.unreachable == loaded.unreachable
                        && r.deleted_branches == loaded.deleted
                        && r.commits == loaded.commits;
                    if unchanged {
                        return (reload, None);
                    }
                    // keep the selected entry (it moves down as lines are added)
                    let selected = r
                        .selected
                        .and_then(|i| r.entries.get(i))
                        .and_then(|old| loaded.entries.iter().position(|e| same_entry(e, old)));
                    let first_load = !r.loaded;
                    r.entries = loaded.entries;
                    r.branch = loaded.branch;
                    r.commits = loaded.commits;
                    r.unreachable = loaded.unreachable;
                    r.deleted_branches = loaded.deleted;
                    r.loaded = true;
                    r.rebuild_rows();
                    r.selected = selected;
                    let select = if first_load || r.selected.is_none() {
                        // the newest entry whose commit is still there
                        r.rows
                            .iter()
                            .map(|row| row.entry)
                            .find(|&e| r.has_commit(e))
                    } else {
                        None
                    };
                    cx.notify();
                    (reload, select)
                });
                if let Some(entry) = select {
                    Self::select_reflog_entry(id, entry, cx);
                }
                if reload {
                    Self::load_reflog(id, cx);
                }
            });
        })
        .detach();
    }

    /// Select entry `entry`: the commit view shows the commit it points at.
    pub fn select_reflog_entry(id: u64, entry: usize, cx: &mut dyn Host) {
        let sha = Self::state(cx).update(cx, |s, cx| {
            let r = s.repo_state_mut(id).reflog.as_mut()?;
            if !r.has_commit(entry) {
                return None;
            }
            if r.selected != Some(entry) {
                r.selected = Some(entry);
                cx.notify();
            }
            Some(r.entries[entry].new.clone())
        });
        if let Some(sha) = sha {
            Self::select_commits(id, vec![sha], cx);
        }
    }

    /// The header's HEAD / branch switch.
    pub fn set_reflog_source(id: u64, source: ReflogSource, cx: &mut dyn Host) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let Some(r) = s.repo_state_mut(id).reflog.as_mut() else {
                return false;
            };
            if r.source == source {
                return false;
            }
            r.source = source;
            r.selected = None;
            r.loaded = false;
            cx.notify();
            true
        });
        if changed {
            Self::load_reflog(id, cx);
        }
    }

    /// "Show every step" / "Only unreachable".
    pub fn set_reflog_options(
        id: u64,
        show_steps: bool,
        only_unreachable: bool,
        cx: &mut dyn Host,
    ) {
        let select = Self::state(cx).update(cx, |s, cx| {
            let r = s.repo_state_mut(id).reflog.as_mut()?;
            r.show_steps = show_steps;
            r.only_unreachable = only_unreachable;
            r.rebuild_rows();
            cx.notify();
            // the selected entry was filtered out: the first one left
            if r.selected.is_some_and(|e| r.row_of(e).is_some()) {
                return None;
            }
            r.rows
                .iter()
                .map(|row| row.entry)
                .find(|&e| r.has_commit(e))
        });
        if let Some(entry) = select {
            Self::select_reflog_entry(id, entry, cx);
        }
    }

    /// Reset Current Branch to Here…: `reset --hard`, confirmed first
    /// (`Popup::ResetToReflogEntry`), uncommitted changes stashed.
    pub fn request_reflog_reset(id: u64, sha: String, cx: &mut dyn Host) {
        let (branch, dirty) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            (
                rs.and_then(|r| r.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                rs.and_then(|r| r.status.as_deref())
                    .map_or(0, |st| st.files.len()),
            )
        };
        Self::show_popup(
            Popup::ResetToReflogEntry {
                repo: id,
                sha,
                branch,
                dirty,
            },
            cx,
        );
    }

    /// `ResetToReflogEntry` › Reset (or Stash and Reset): stash the changes
    /// on the current branch when `stash`, then `reset --hard <sha>`. The
    /// list stays open; the commits left behind are in it.
    pub fn reflog_reset(id: u64, sha: String, stash: bool, cx: &mut dyn Host) {
        let branch = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.info.as_ref())
            .and_then(|i| i.current_branch())
            .map(|b| b.name.clone());
        let guard = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED);
        Self::close_popup(cx);
        Self::run_history_op(
            id,
            "Could not reset",
            move |git, workdir| {
                if stash {
                    let Some(branch) = branch else {
                        return Err(corvene_git::GitError::Gix(
                            "changes can only be stashed on a branch".to_string(),
                        ));
                    };
                    corvene_git::create_desktop_stash(git.clone(), &workdir, &branch, guard)?;
                }
                corvene_git::reset_to(git, &workdir, corvene_git::ResetMode::Hard, &sha)
            },
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(message: &str, new: &str) -> ReflogEntry {
        ReflogEntry {
            old: "a".repeat(40),
            new: new.to_string(),
            seconds: 0,
            offset: 0,
            message: message.to_string(),
            action: corvene_git::parse_reflog_action(message),
        }
    }

    #[test]
    fn folds_rebases_unless_asked() {
        let entries = vec![
            entry("commit: after", "1"),
            entry("rebase (finish): returning to refs/heads/topic", "2"),
            entry("rebase (pick): two", "3"),
            entry("rebase (pick): one", "4"),
            entry("rebase (start): checkout main", "5"),
            entry("commit: before", "6"),
        ];
        let rows = build_rows(&entries, false, None);
        assert_eq!(
            rows,
            vec![
                ReflogRow {
                    entry: 0,
                    folded: 0
                },
                ReflogRow {
                    entry: 1,
                    folded: 3
                },
                ReflogRow {
                    entry: 5,
                    folded: 0
                },
            ]
        );
        assert_eq!(build_rows(&entries, true, None).len(), 6);
        let unreachable: HashSet<String> = ["2".to_string(), "6".to_string()].into();
        let rows = build_rows(&entries, false, Some(&unreachable));
        assert_eq!(rows.iter().map(|r| r.entry).collect::<Vec<_>>(), vec![1, 5]);
    }

    #[test]
    fn finds_deleted_branches() {
        let sha = "b".repeat(40);
        let entries = vec![
            entry("checkout: moving from gone to main", "1"),
            entry("checkout: moving from main to gone", "2"),
            entry("checkout: moving from gone to main", "3"),
            entry(&format!("checkout: moving from {sha} to main"), "4"),
            entry("checkout: moving from kept to main", "5"),
        ];
        let existing: HashSet<String> = ["main".to_string(), "kept".to_string()].into();
        let found = deleted_branches(&entries, &existing);
        assert_eq!(found, HashMap::from([(0, "gone".to_string())]));
    }
}
