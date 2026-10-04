//! Compare to branch - GHD `ICompareState` (`lib/app-state.ts`) and
//! `_executeCompare` / `updateCompareToBranch` / `_updateCompareForm` in
//! `app-store.ts`: the History tab either shows the branch's history or the
//! commits the current branch is behind / ahead of another branch, with the
//! merge call to action.
//!
//! Deviation (`889-compare-shows-conflicts`): both tabs also list the files
//! a merge of the compared branch would leave conflicted (GHD only counts
//! them in the Behind tab's merge call to action).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use corvene_models::{AheadBehind, Commit, Mergeability};
use gpui_kit::{App, AsyncApp};
use tracing::warn;

use crate::ahead_behind_store::Disposable;
use crate::dispatcher::Dispatcher;

/// `ComparisonMode`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonMode {
    /// Commits on the compared branch that are not on the current one.
    Behind,
    /// Commits on the current branch that are not on the compared one.
    Ahead,
}

/// `IDisplayHistory | ICompareBranch`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompareForm {
    History,
    Branch {
        branch: String,
        mode: ComparisonMode,
        ahead_behind: AheadBehind,
    },
}

/// `ICompareState`
#[derive(Clone, Debug, PartialEq)]
pub struct CompareState {
    pub form: CompareForm,
    /// `filterText`: the compare box's text (the compared branch's name
    /// while comparing, empty in the plain history).
    pub filter_text: String,
    /// `showBranchList`: the compare box is expanded into the branch list.
    pub show_branch_list: bool,
    /// The comparison commits (`commitSHAs` while in `Branch` mode).
    pub commits: Vec<Commit>,
    pub loading: bool,
    /// `mergeStatus` for the merge call to action; `None` while loading.
    pub merge_status: Option<Mergeability>,
    /// `889-compare-shows-conflicts`: the files merging the compared branch
    /// would leave conflicted.
    pub conflicted_files: Vec<String>,
    /// Ahead/behind of every other branch relative to the current one
    /// (`AheadBehindStore`), filled while the list is open.
    pub branch_counts: HashMap<String, AheadBehind>,
    pub counts_loaded: bool,
    /// The `AppState::ahead_behind` requests filling `branch_counts`,
    /// disposed when a newer load replaces them.
    pub counts_requests: Vec<Disposable>,
    /// Flag `825`: the repository's tags, loaded with the counts, offered
    /// in the list while filtering.
    pub tags: Vec<String>,
}

impl Default for CompareState {
    fn default() -> Self {
        Self {
            form: CompareForm::History,
            filter_text: String::new(),
            show_branch_list: false,
            commits: Vec::new(),
            loading: false,
            merge_status: None,
            conflicted_files: Vec::new(),
            branch_counts: HashMap::new(),
            counts_loaded: false,
            counts_requests: Vec::new(),
            tags: Vec::new(),
        }
    }
}

impl CompareState {
    pub fn is_comparing(&self) -> bool {
        matches!(self.form, CompareForm::Branch { .. })
    }

    pub fn branch(&self) -> Option<&str> {
        match &self.form {
            CompareForm::Branch { branch, .. } => Some(branch),
            CompareForm::History => None,
        }
    }
}

impl Dispatcher {
    /// `updateCompareForm({ filterText })`: the compare box was edited.
    pub fn set_compare_filter_text(id: u64, filter_text: String, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.compare.filter_text != filter_text {
                rs.compare.filter_text = filter_text;
                cx.notify();
            }
        });
    }

    /// `updateCompareForm({ showBranchList })`
    pub fn set_compare_branch_list_visible(id: u64, visible: bool, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.compare.show_branch_list != visible {
                rs.compare.show_branch_list = visible;
                cx.notify();
            }
        });
        if visible {
            Self::load_compare_counts(id, cx);
        }
    }

    /// Ahead/behind counters for the compare branch list: GHD's
    /// `AheadBehindStore.getAheadBehind(repository, currentTip, branchTip)`
    /// for every other branch (`AppState::ahead_behind`, which caches them by
    /// tip shas across refreshes and counts the rest one `rev-list
    /// --left-right --count` at a time). The cached counts show at once and
    /// the others together once all are counted; GHD's rows subscribe while
    /// they are rendered and fill in one by one.
    fn load_compare_counts(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (current_tip, branches, loaded, with_tags) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(info) = rs.info.as_ref() else { return };
            let Some(current) = info.current_branch() else {
                return;
            };
            let Some(current_tip) = current.tip.clone() else {
                return;
            };
            (
                current_tip,
                info.branches
                    .iter()
                    .filter(|b| b.full_name != current.full_name)
                    .filter_map(|b| Some((b.name.clone(), b.tip.clone()?)))
                    .collect::<Vec<_>>(),
                rs.compare.counts_loaded,
                s.flags.bool(crate::flags::ids::COMPARE_TAGS),
            )
        };
        if loaded {
            return;
        }
        let counts: Rc<RefCell<HashMap<String, AheadBehind>>> = Rc::default();
        // this load's own handle: a newer load disposes it with the requests
        let load = Disposable::new();
        let waiter = Self::state(cx).update(cx, |s, cx| {
            let previous = std::mem::take(&mut s.repo_state_mut(id).compare.counts_requests);
            for request in previous {
                request.dispose();
            }
            let mut requests = vec![load.clone()];
            for (name, tip) in branches {
                let counts = counts.clone();
                requests.push(s.ahead_behind.get_ahead_behind(
                    git.clone(),
                    &workdir,
                    &current_tip,
                    &tip,
                    move |ab| {
                        counts.borrow_mut().insert(name, ab);
                    },
                ));
            }
            let compare = &mut s.repo_state_mut(id).compare;
            compare.counts_loaded = true;
            compare.counts_requests = requests;
            // the cached counts answered at once
            compare.branch_counts = counts.borrow_mut().drain().collect();
            cx.notify();
            s.ahead_behind.waiter()
        });
        let task = cx.background_executor().spawn(async move {
            let tags = if with_tags {
                corvene_git::tag_names(&workdir).unwrap_or_else(|err| {
                    warn!(%err, "compare: could not list tags");
                    Vec::new()
                })
            } else {
                Vec::new()
            };
            waiter.wait_idle();
            tags
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let tags = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    // runs the callbacks of every counted range
                    s.ahead_behind.poll();
                    if load.disposed() {
                        return;
                    }
                    let compare = &mut s.repo_state_mut(id).compare;
                    compare.branch_counts.extend(counts.borrow_mut().drain());
                    compare.tags = tags;
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// `executeCompare({ kind: Compare, branch, comparisonMode })`
    pub fn compare_to_branch(id: u64, branch: String, mode: ComparisonMode, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((current, _)) = Self::current_branch_and_tip_pub(id, cx) else {
            return;
        };
        let list_conflicts = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::COMPARE_SHOWS_CONFLICTS);
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.compare.loading = true;
            rs.compare.show_branch_list = false;
            cx.notify();
        });
        let branch_for_task = branch.clone();
        let current_for_task = current.clone();
        let task = cx.background_executor().spawn(async move {
            let ahead_behind = corvene_git::symmetric_ahead_behind(
                git.clone(),
                &workdir,
                &current_for_task,
                &branch_for_task,
            )?;
            let Some(ahead_behind) = ahead_behind else {
                return Ok(None);
            };
            let (from, to, count) = match mode {
                ComparisonMode::Behind => {
                    (&current_for_task, &branch_for_task, ahead_behind.behind)
                }
                ComparisonMode::Ahead => (&branch_for_task, &current_for_task, ahead_behind.ahead),
            };
            let commits = corvene_git::get_commits_in_range(&workdir, from, to, count as usize)?;
            // `889`: the conflicted files, in either tab (the counted
            // merge status comes from the same merge-tree)
            let conflicts = (list_conflicts && ahead_behind.behind > 0)
                .then(|| {
                    corvene_git::merge_tree_conflicts(
                        git.clone(),
                        &workdir,
                        &current_for_task,
                        &branch_for_task,
                    )
                    .ok()
                })
                .flatten();
            let merge_status = if mode == ComparisonMode::Behind && ahead_behind.behind > 0 {
                match &conflicts {
                    Some(Some(files)) if files.is_empty() => Some(Mergeability::Clean),
                    Some(Some(files)) => Some(Mergeability::Conflicts(files.len() as u32)),
                    Some(None) => Some(Mergeability::Invalid),
                    None => corvene_git::determine_mergeability(
                        git,
                        &workdir,
                        &current_for_task,
                        &branch_for_task,
                    )
                    .ok(),
                }
            } else {
                None
            };
            let conflicted_files = conflicts.flatten().unwrap_or_default();
            Ok::<_, corvene_git::GitError>(Some((
                ahead_behind,
                commits,
                merge_status,
                conflicted_files,
            )))
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let select = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.compare.loading = false;
                    match result {
                        Ok(Some((ahead_behind, commits, merge_status, conflicted_files))) => {
                            rs.compare.form = CompareForm::Branch {
                                branch: branch.clone(),
                                mode,
                                ahead_behind,
                            };
                            rs.compare.filter_text = branch.clone();
                            rs.compare.commits = commits;
                            rs.compare.merge_status = merge_status;
                            rs.compare.conflicted_files = conflicted_files;
                            let first = rs.compare.commits.first().map(|c| c.sha.clone());
                            let keep = rs
                                .selected_commits
                                .iter()
                                .all(|sha| rs.compare.commits.iter().any(|c| &c.sha == sha))
                                && !rs.selected_commits.is_empty();
                            cx.notify();
                            if keep { None } else { Some(first) }
                        }
                        Ok(None) => {
                            warn!(id, %branch, "compare: branch could not be resolved");
                            cx.notify();
                            None
                        }
                        Err(err) => {
                            warn!(id, %err, "compare failed");
                            cx.notify();
                            None
                        }
                    }
                });
                match select {
                    Some(Some(sha)) => Self::select_commits(id, vec![sha], cx),
                    Some(None) => Self::select_commits(id, Vec::new(), cx),
                    None => {}
                }
            });
        })
        .detach();
    }

    /// The Behind / Ahead tabs.
    pub fn set_comparison_mode(id: u64, mode: ComparisonMode, cx: &mut App) {
        let branch = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.compare.branch().map(str::to_string));
        if let Some(branch) = branch {
            Self::compare_to_branch(id, branch, mode, cx);
        }
    }

    /// `executeCompare({ kind: History })`: back to the branch's history.
    pub fn exit_compare(id: u64, cx: &mut App) {
        let was_comparing = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let was = rs.compare.is_comparing();
            rs.compare.form = CompareForm::History;
            rs.compare.filter_text.clear();
            rs.compare.commits.clear();
            rs.compare.merge_status = None;
            rs.compare.conflicted_files.clear();
            rs.compare.show_branch_list = false;
            cx.notify();
            was
        });
        if was_comparing {
            let first = Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .and_then(|r| r.commits.first().map(|c| c.sha.clone()));
            Self::select_commits(id, first.into_iter().collect(), cx);
        }
    }

    /// After a refresh: re-run an active comparison and forget the cached counters.
    pub(crate) fn refresh_compare(id: u64, cx: &mut App) {
        let active = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            rs.compare.counts_loaded = false;
            match &rs.compare.form {
                CompareForm::Branch { branch, mode, .. } => Some((branch.clone(), *mode)),
                CompareForm::History => None,
            }
        });
        if let Some((branch, mode)) = active {
            Self::compare_to_branch(id, branch, mode, cx);
        }
    }

    /// Merge call to action: merge / squash-merge / rebase onto the compared branch.
    pub fn compare_merge_action(
        id: u64,
        kind: corvene_models::MultiCommitOperationKind,
        cx: &mut App,
    ) {
        if Self::refuse_merge_while_conflicted(id, cx) {
            return;
        }
        let Some(branch) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.compare.branch().map(str::to_string))
        else {
            return;
        };
        Self::exit_compare(id, cx);
        match kind {
            corvene_models::MultiCommitOperationKind::Rebase => {
                Self::start_rebase_flow(id, cx);
                Self::preview_rebase(id, branch.clone(), cx);
                Self::start_rebase(id, branch, false, cx);
            }
            corvene_models::MultiCommitOperationKind::Squash => {
                Self::merge_branch(id, branch, true, cx)
            }
            _ => Self::merge_branch(id, branch, false, cx),
        }
    }
}
