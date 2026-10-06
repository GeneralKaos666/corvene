//! Corvene addition (`1313-changelists`, desktop/desktop#16925): named
//! groups of changed files in the Changes tab, as JetBrains and Perforce
//! changelists. GHD has one flat list of changed files
//! (`ui/changes/filter-changes-list.tsx`) and no grouping.
//!
//! A [`Changelist`] is a name, an optional description and a set of paths.
//! Membership is by path and stays while a file is temporarily unchanged
//! (stashed, or not touched again yet), so a stash of a list's files puts
//! them back into the list when it is restored; a path leaves its list when
//! it is committed or discarded through Corvene, or moved by the user.
//! Files in no list form the plain "Changes" group, shown last. A list may
//! be the *active* one: new changes join it, and the commit button commits
//! only its ticked files (the JetBrains model); without an active list the
//! checkboxes alone decide, and a list's header checkbox ticks or unticks
//! its files. Both models are persisted per repository
//! ([`crate::persistence::StoreExt::changelists`], store key
//! `changes.changelists`), as the collapsed folders of `1310` are.
//!
//! Stashes made from a list ([`Dispatcher::stash_changelist`], and the
//! lists left behind by a branch switch,
//! [`Dispatcher::checkout_branch_splitting_changelists`]) are remembered by
//! sha ([`Changelists::stashed`]) with the list as it was, so the stash list
//! can name them and a restore re-creates a list deleted meanwhile.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use corvene_models::{DiffSelectionType, WorkingDirectoryFileChange};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::{Host, StateCx};
use crate::persistence::StoreExt;
use crate::state::AppState;

/// The id of the plain group: files in no list.
pub const UNGROUPED: u64 = 0;

/// What became of a stash made from lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashFate {
    /// Popped: its lists come back when deleted, and the record goes.
    Restored,
    /// Applied, the entry kept: its lists come back when deleted.
    Applied,
    /// Dropped: the record goes.
    Dropped,
}

/// A named group of changed files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changelist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Repository-relative paths, as the status lists them.
    #[serde(default)]
    pub paths: BTreeSet<String>,
}

/// A list as it was when its files were stashed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StashedChangelist {
    pub list: u64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub paths: BTreeSet<String>,
}

/// A repository's changelists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changelists {
    /// In the order they show; ids start at 1 ([`UNGROUPED`] is 0).
    pub lists: Vec<Changelist>,
    /// The active list: new changes join it and only its ticked files are
    /// committed.
    #[serde(default)]
    pub active: Option<u64>,
    /// Collapsed groups, [`UNGROUPED`] included.
    #[serde(default)]
    pub collapsed: BTreeSet<u64>,
    #[serde(default)]
    next_id: u64,
    /// Stashes made from lists, by stash sha.
    #[serde(default)]
    pub stashed: BTreeMap<String, Vec<StashedChangelist>>,
}

impl Changelists {
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&Changelist> {
        self.lists.iter().find(|l| l.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Changelist> {
        self.lists.iter_mut().find(|l| l.id == id)
    }

    /// The active list, when it still exists.
    pub fn active_list(&self) -> Option<&Changelist> {
        self.active.and_then(|id| self.get(id))
    }

    /// The group `path` is in: its list's id, else [`UNGROUPED`].
    pub fn group_of(&self, path: &str) -> u64 {
        self.lists
            .iter()
            .find(|l| l.paths.contains(path))
            .map_or(UNGROUPED, |l| l.id)
    }

    /// The paths of `files` in the active list (every path without one).
    /// What the commit button commits is the ticked files among these.
    pub fn commits_path(&self, path: &str) -> bool {
        match self.active_list() {
            Some(list) => list.paths.contains(path),
            None => true,
        }
    }

    /// Whether `name` (trimmed) is taken by a list other than `except`.
    pub fn name_taken(&self, name: &str, except: Option<u64>) -> bool {
        let name = name.trim();
        self.lists
            .iter()
            .any(|l| Some(l.id) != except && l.name.eq_ignore_ascii_case(name))
    }

    /// A new list holding `paths` (taken out of their old lists); its id.
    pub fn create(
        &mut self,
        name: &str,
        description: &str,
        paths: impl IntoIterator<Item = String>,
    ) -> u64 {
        self.next_id = self
            .next_id
            .max(self.lists.iter().map(|l| l.id).max().unwrap_or(0));
        self.next_id += 1;
        let id = self.next_id;
        let paths: BTreeSet<String> = paths.into_iter().collect();
        for list in &mut self.lists {
            list.paths.retain(|p| !paths.contains(p));
        }
        self.lists.push(Changelist {
            id,
            name: name.trim().to_string(),
            description: description.trim().to_string(),
            paths,
        });
        id
    }

    /// Rename `id` and set its description; false when there is no such
    /// list.
    pub fn update(&mut self, id: u64, name: &str, description: &str) -> bool {
        match self.get_mut(id) {
            Some(list) => {
                list.name = name.trim().to_string();
                list.description = description.trim().to_string();
                true
            }
            None => false,
        }
    }

    /// Delete `id`; its files become ungrouped.
    pub fn delete(&mut self, id: u64) {
        self.lists.retain(|l| l.id != id);
        self.collapsed.remove(&id);
        if self.active == Some(id) {
            self.active = None;
        }
    }

    /// Put `paths` into list `to` (`None` or [`UNGROUPED`]: into no list).
    pub fn move_paths(&mut self, paths: &[String], to: Option<u64>) {
        let set: HashSet<&str> = paths.iter().map(String::as_str).collect();
        for list in &mut self.lists {
            list.paths.retain(|p| !set.contains(p.as_str()));
        }
        if let Some(to) = to.filter(|&to| to != UNGROUPED)
            && let Some(list) = self.get_mut(to)
        {
            list.paths.extend(paths.iter().cloned());
        }
    }

    /// Take `paths` out of every list (committed or discarded).
    pub fn remove_paths<'a>(&mut self, paths: impl IntoIterator<Item = &'a str>) -> bool {
        let set: HashSet<&str> = paths.into_iter().collect();
        if set.is_empty() {
            return false;
        }
        let mut changed = false;
        for list in &mut self.lists {
            let before = list.paths.len();
            list.paths.retain(|p| !set.contains(p.as_str()));
            changed |= list.paths.len() != before;
        }
        changed
    }

    /// New changes join the active list.
    pub fn capture_new_paths(&mut self, paths: impl IntoIterator<Item = String>) -> bool {
        let Some(active) = self.active else {
            return false;
        };
        let taken: HashSet<String> = self
            .lists
            .iter()
            .flat_map(|l| l.paths.iter().cloned())
            .collect();
        let Some(list) = self.get_mut(active) else {
            return false;
        };
        let mut changed = false;
        for path in paths {
            if !taken.contains(&path) {
                changed |= list.paths.insert(path);
            }
        }
        changed
    }

    /// Make `id` the active list (`None`: none).
    pub fn set_active(&mut self, id: Option<u64>) {
        self.active = id.filter(|&id| self.get(id).is_some());
    }

    pub fn set_collapsed(&mut self, group: u64, collapsed: bool) -> bool {
        if collapsed {
            self.collapsed.insert(group)
        } else {
            self.collapsed.remove(&group)
        }
    }

    /// Remember that stash `sha` holds the files of `lists`.
    pub fn note_stashed(&mut self, sha: &str, lists: Vec<StashedChangelist>) {
        if !lists.is_empty() {
            self.stashed.insert(sha.to_string(), lists);
        }
    }

    /// The lists stash `sha` was made from.
    pub fn stashed_lists(&self, sha: &str) -> &[StashedChangelist] {
        self.stashed.get(sha).map_or(&[], Vec::as_slice)
    }

    /// Stash `sha` came back: the lists it was made from are re-created
    /// when they were deleted meanwhile (their files are members again
    /// either way). Returns whether a list was re-created.
    pub fn revive_stashed(&mut self, sha: &str) -> bool {
        let Some(lists) = self.stashed.get(sha).cloned() else {
            return false;
        };
        let mut revived = false;
        for stashed in lists {
            if self.get(stashed.list).is_some() {
                continue;
            }
            let id = self.create(&stashed.name, &stashed.description, stashed.paths);
            // keep the old id when it is free, so other stashes of the same
            // list still find it
            if self.get(stashed.list).is_none()
                && let Some(list) = self.get_mut(id)
            {
                list.id = stashed.list;
            }
            revived = true;
        }
        revived
    }

    /// Stash `sha` is gone (restored or dropped): forget its lists.
    /// `1315-discard-stash-file`: stash `old` was rewritten as `new`.
    pub fn rekey_stashed(&mut self, old: &str, new: &str) -> bool {
        let Some(lists) = self.stashed.remove(old) else {
            return false;
        };
        self.stashed.insert(new.to_string(), lists);
        true
    }

    pub fn forget_stashed(&mut self, sha: &str) -> bool {
        self.stashed.remove(sha).is_some()
    }

    /// What became of a stash made from lists, for [`Self::note_fate`].
    pub fn note_fate(&mut self, sha: &str, fate: StashFate) -> bool {
        match fate {
            StashFate::Restored => {
                let revived = self.revive_stashed(sha);
                self.forget_stashed(sha) || revived
            }
            StashFate::Applied => self.revive_stashed(sha),
            StashFate::Dropped => self.forget_stashed(sha),
        }
    }

    /// The snapshot of list `id` to record with a stash of its files.
    pub fn snapshot(&self, id: u64) -> Option<StashedChangelist> {
        let list = self.get(id)?;
        Some(StashedChangelist {
            list: list.id,
            name: list.name.clone(),
            description: list.description.clone(),
            paths: list.paths.clone(),
        })
    }
}

/// What the stash list calls stash `sha` when it was made from lists:
/// "Stash of <name>" (or the names joined), else `None`.
pub fn stash_label(lists: &Changelists, sha: &str) -> Option<String> {
    let stashed = lists.stashed_lists(sha);
    if stashed.is_empty() {
        return None;
    }
    let names: Vec<&str> = stashed.iter().map(|s| s.name.as_str()).collect();
    Some(format!("Stash of {}", names.join(", ")))
}

/// Whether the flag is on.
pub fn enabled(s: &AppState) -> bool {
    s.flags.bool(crate::flags::ids::CHANGELISTS)
}

/// Repository `id`'s lists (`None` when it has none, or the flag is off).
pub fn of(s: &AppState, id: u64) -> Option<Arc<Changelists>> {
    if !enabled(s) {
        return None;
    }
    s.changelists.get(&id).cloned()
}

/// Repository `id`'s lists for the menus: an empty set while it has none,
/// `None` only with the flag off.
pub fn for_menu(s: &AppState, id: u64) -> Option<Arc<Changelists>> {
    if !enabled(s) {
        return None;
    }
    Some(s.changelists.get(&id).cloned().unwrap_or_default())
}

/// The named lists of repository `id` with how many of `status`'s files
/// each holds, for the Switch Branch dialog (lists with none are left out).
pub fn lists_with_changes(
    s: &AppState,
    id: u64,
    status: Option<&corvene_models::WorkingDirectoryStatus>,
) -> Vec<(u64, String, usize)> {
    let Some(lists) = of(s, id) else {
        return Vec::new();
    };
    let Some(status) = status else {
        return Vec::new();
    };
    lists
        .lists
        .iter()
        .map(|l| {
            let n = status
                .files
                .iter()
                .filter(|f| l.paths.contains(&f.path))
                .count();
            (l.id, l.name.clone(), n)
        })
        .filter(|(_, _, n)| *n > 0)
        .collect()
}

/// Whether `file` goes into the next commit: ticked, and in the active list
/// when there is one.
pub fn committed(lists: Option<&Changelists>, file: &WorkingDirectoryFileChange) -> bool {
    file.selection.kind() != DiffSelectionType::None
        && lists.is_none_or(|l| l.commits_path(&file.path))
}

/// The snapshot each save takes, and the newest one written.
static SAVE_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SAVED_GENERATION: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

/// Edit repository `id`'s lists in place and save them when `edit` returns
/// true; a repository without lists gets a fresh set.
pub(crate) fn edit(
    s: &mut AppState,
    id: u64,
    cx: &mut StateCx,
    edit: impl FnOnce(&mut Changelists) -> bool,
) -> bool {
    let mut lists = s
        .changelists
        .get(&id)
        .map(|l| (**l).clone())
        .unwrap_or_default();
    if !edit(&mut lists) {
        return false;
    }
    if lists == Changelists::default() {
        s.changelists.remove(&id);
    } else {
        s.changelists.insert(id, Arc::new(lists));
    }
    cx.notify();
    save(s);
    true
}

/// Write every repository's lists on a thread of their own; a later
/// snapshot that got written first is not overwritten by an older one.
fn save(s: &AppState) {
    let store = s.store.clone();
    let saved: HashMap<u64, Changelists> = s
        .changelists
        .iter()
        .map(|(id, lists)| (*id, (**lists).clone()))
        .collect();
    let generation = SAVE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::Builder::new()
        .name("changelists".into())
        .spawn(move || {
            let Ok(mut written) = SAVED_GENERATION.lock() else {
                return;
            };
            if *written > generation {
                return;
            }
            *written = generation;
            if let Err(err) = store.save_changelists(&saved) {
                warn!(?err, "could not save the changelists");
            }
        })
        .map(drop)
        .unwrap_or_else(|err| warn!(?err, "could not save the changelists"));
}

/// Files of `status` whose paths are in `paths`, each with all of its
/// changes selected (a list is stashed or left behind whole).
fn whole_files(
    status: Option<&corvene_models::WorkingDirectoryStatus>,
    paths: &BTreeSet<String>,
) -> Vec<WorkingDirectoryFileChange> {
    status
        .map(|st| {
            st.files
                .iter()
                .filter(|f| paths.contains(&f.path))
                .cloned()
                .map(|mut f| {
                    f.selection = f.selection.select_all();
                    f
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Dispatcher {
    /// New Changelist…: a list named `name` holding `paths`; `active` makes
    /// it the active list.
    pub fn create_changelist(
        id: u64,
        name: String,
        description: String,
        paths: Vec<String>,
        active: bool,
        cx: &mut dyn Host,
    ) {
        if name.trim().is_empty() {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| {
                let new = lists.create(&name, &description, paths);
                if active {
                    lists.set_active(Some(new));
                }
                true
            });
        });
    }

    /// Edit Changelist…: rename `list` and set its description.
    pub fn update_changelist(
        id: u64,
        list: u64,
        name: String,
        description: String,
        cx: &mut dyn Host,
    ) {
        if name.trim().is_empty() {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| lists.update(list, &name, &description));
        });
    }

    /// Delete Changelist: its files become ungrouped.
    pub fn delete_changelist(id: u64, list: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| {
                let had = lists.get(list).is_some();
                lists.delete(list);
                had
            });
        });
    }

    /// Move to Changelist ▸: `paths` go into `list` (`None`: out of every
    /// list).
    pub fn move_to_changelist(id: u64, paths: Vec<String>, list: Option<u64>, cx: &mut dyn Host) {
        if paths.is_empty() {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| {
                lists.move_paths(&paths, list);
                true
            });
        });
    }

    /// Set as Active Changelist / Clear Active Changelist.
    pub fn set_active_changelist(id: u64, list: Option<u64>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| {
                let before = lists.active;
                lists.set_active(list);
                lists.active != before
            });
        });
    }

    /// Collapse or expand group `group` ([`UNGROUPED`] for the plain one).
    pub fn set_changelist_collapsed(id: u64, group: u64, collapsed: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| lists.set_collapsed(group, collapsed));
        });
    }

    /// Commit Only This List: tick every file of `list` (partly ticked ones
    /// keep their lines) and untick every other file. Another list that is
    /// active hands the role to this one, else nothing would be committed.
    pub fn check_only_changelist(id: u64, list: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            let Some(paths) = s
                .changelists
                .get(&id)
                .and_then(|l| l.get(list))
                .map(|l| l.paths.clone())
            else {
                return;
            };
            if s.changelists
                .get(&id)
                .is_some_and(|l| l.active.is_some_and(|a| a != list))
            {
                edit(s, id, cx, |lists| {
                    lists.set_active(Some(list));
                    true
                });
            }
            if let Some(status) = s.repo_state_mut(id).status.as_mut().map(Arc::make_mut) {
                for f in &mut status.files {
                    let inside = paths.contains(&f.path);
                    if inside && f.selection.kind() == DiffSelectionType::None {
                        f.selection = f.selection.select_all();
                    } else if !inside {
                        f.selection = f.selection.select_none();
                    }
                }
                crate::drafts::note_excluded(s, id, cx);
                cx.notify();
            }
        });
    }

    /// Stash This List: a stash of every change to the list's files
    /// ([`crate::stash_flows::PartialStash`] decides which kind), remembered
    /// with the list so the stash list names it and a restore brings the
    /// list back if it was deleted meanwhile.
    pub fn stash_changelist(id: u64, list: u64, cx: &mut dyn Host) {
        let (files, snapshot) = {
            let s = Self::state(cx).read(cx);
            let Some(lists) = s.changelists.get(&id) else {
                return;
            };
            let Some(snapshot) = lists.snapshot(list) else {
                return;
            };
            let status = s.repo_states.get(&id).and_then(|rs| rs.status.as_deref());
            (whole_files(status, &snapshot.paths), snapshot)
        };
        if files.is_empty() {
            return;
        }
        let message = snapshot.name.clone();
        Self::stash_partial_then(
            id,
            files,
            Some(message),
            move |sha, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    edit(s, id, cx, |lists| {
                        lists.note_stashed(&sha, vec![snapshot]);
                        true
                    });
                });
            },
            cx,
        );
    }

    /// A stash made from lists was restored, applied or dropped
    /// ([`StashFate`]): deleted lists come back, or the record goes.
    pub(crate) fn note_changelist_stash_fate(
        id: u64,
        sha: &str,
        fate: StashFate,
        cx: &mut dyn Host,
    ) {
        let known = Self::state(cx)
            .read(cx)
            .changelists
            .get(&id)
            .is_some_and(|l| l.stashed.contains_key(sha));
        if !known {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| lists.note_fate(sha, fate));
        });
    }

    /// `1315-discard-stash-file`: the lists recorded with stash `old` move
    /// to its rewritten commit `new`.
    pub(crate) fn rekey_changelist_stash(id: u64, old: &str, new: &str, cx: &mut dyn Host) {
        let known = Self::state(cx)
            .read(cx)
            .changelists
            .get(&id)
            .is_some_and(|l| l.stashed.contains_key(old));
        if !known {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| lists.rekey_stashed(old, new));
        });
    }

    /// After a status arrived: new changed paths (in `status`, not in
    /// `previous`) join the active list.
    pub(crate) fn capture_new_changelist_paths(
        s: &mut AppState,
        id: u64,
        new_paths: Vec<String>,
        cx: &mut StateCx,
    ) {
        if new_paths.is_empty() || !enabled(s) {
            return;
        }
        if !s
            .changelists
            .get(&id)
            .is_some_and(|l| l.active_list().is_some())
        {
            return;
        }
        edit(s, id, cx, |lists| lists.capture_new_paths(new_paths));
    }

    /// `paths` were committed or discarded: they leave their lists.
    pub(crate) fn forget_changelist_paths(id: u64, paths: &[String], cx: &mut dyn Host) {
        if paths.is_empty() {
            return;
        }
        let has = Self::state(cx)
            .read(cx)
            .changelists
            .get(&id)
            .is_some_and(|l| !l.is_empty());
        if !has {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            edit(s, id, cx, |lists| {
                lists.remove_paths(paths.iter().map(String::as_str))
            });
        });
    }

    /// The paths the next commit takes: ticked files, in the active list
    /// when there is one.
    pub(crate) fn committed_files(s: &AppState, id: u64) -> Vec<WorkingDirectoryFileChange> {
        let lists = of(s, id);
        s.repo_states
            .get(&id)
            .and_then(|r| r.status.as_deref())
            .map(|st| {
                st.files
                    .iter()
                    .filter(|f| committed(lists.as_deref(), f))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Switch Branch with some lists left behind: the files of `leave`
    /// (and, with `leave_rest`, every file in no other list) go into the
    /// branch's Desktop stash, then the checkout carries the rest. A
    /// Desktop stash the branch has is replaced, as "Leave my changes"
    /// replaces it.
    pub fn checkout_branch_splitting_changelists(
        id: u64,
        branch: String,
        leave: Vec<u64>,
        leave_rest: bool,
        cx: &mut dyn Host,
    ) {
        let (files, snapshots, current, previous_stash, guard) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(lists) = s.changelists.get(&id) else {
                return;
            };
            let mut paths: BTreeSet<String> = BTreeSet::new();
            let mut snapshots = Vec::new();
            for list in &leave {
                if let Some(snapshot) = lists.snapshot(*list) {
                    paths.extend(snapshot.paths.iter().cloned());
                    snapshots.push(snapshot);
                }
            }
            if leave_rest && let Some(st) = rs.status.as_deref() {
                paths.extend(
                    st.files
                        .iter()
                        .filter(|f| lists.group_of(&f.path) == UNGROUPED)
                        .map(|f| f.path.clone()),
                );
            }
            (
                whole_files(rs.status.as_deref(), &paths),
                snapshots,
                rs.info
                    .as_ref()
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                rs.desktop_stash().map(|s| s.sha.clone()),
                s.flags
                    .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED),
            )
        };
        let Some(current) = current else {
            return;
        };
        if files.is_empty() {
            // nothing stays: a plain "bring my changes" switch
            Self::checkout_branch(
                id,
                branch,
                Some(crate::persistence::UncommittedChangesStrategy::MoveToNewBranch),
                cx,
            );
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let stash_branch = current.clone();
        crate::remote::spawn_bg(
            cx,
            move || {
                let made = corvene_git::create_desktop_stash_of_files(
                    git.clone(),
                    &workdir,
                    &stash_branch,
                    &files,
                    guard,
                )?;
                if made && let Some(old) = previous_stash {
                    let _ = corvene_git::drop_desktop_stash_entry(git.clone(), &workdir, &old);
                }
                let sha = made
                    .then(|| {
                        corvene_git::get_last_desktop_stash_entry_for_branch(
                            git,
                            &workdir,
                            &stash_branch,
                        )
                    })
                    .transpose()?
                    .flatten()
                    .map(|e| e.sha);
                Ok::<_, corvene_git::GitError>(sha)
            },
            move |result, cx| match result {
                Ok(sha) => {
                    if let Some(sha) = sha {
                        Self::state(cx).update(cx, |s, cx| {
                            edit(s, id, cx, |lists| {
                                lists.note_stashed(&sha, snapshots);
                                true
                            });
                        });
                    }
                    Self::checkout_branch(
                        id,
                        branch,
                        Some(crate::persistence::UncommittedChangesStrategy::MoveToNewBranch),
                        cx,
                    );
                }
                Err(err) => {
                    Self::show_error("Could not stash changes", &err, cx);
                    Self::refresh_repository(id, cx);
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lists() -> Changelists {
        let mut lists = Changelists::default();
        lists.create("Config", "Local settings", ["a.toml".to_string()]);
        lists.create(
            "Feature",
            "",
            ["src/x.rs".to_string(), "src/y.rs".to_string()],
        );
        lists
    }

    #[test]
    fn create_takes_paths_out_of_other_lists() {
        let mut l = lists();
        let id = l.create("Later", "", ["src/x.rs".to_string()]);
        assert_eq!(l.group_of("src/x.rs"), id);
        assert_eq!(l.get(2).map(|f| f.paths.len()), Some(1));
        assert_eq!(l.group_of("README"), UNGROUPED);
    }

    #[test]
    fn ids_are_never_reused() {
        let mut l = lists();
        l.delete(2);
        let id = l.create("Again", "", Vec::<String>::new());
        assert_eq!(id, 3);
    }

    #[test]
    fn move_paths_to_none_ungroups() {
        let mut l = lists();
        l.move_paths(&["a.toml".to_string()], None);
        assert_eq!(l.group_of("a.toml"), UNGROUPED);
        l.move_paths(&["a.toml".to_string()], Some(2));
        assert_eq!(l.group_of("a.toml"), 2);
        l.move_paths(&["a.toml".to_string()], Some(UNGROUPED));
        assert_eq!(l.group_of("a.toml"), UNGROUPED);
    }

    #[test]
    fn committed_paths_leave_their_lists_and_the_empty_list_stays() {
        let mut l = lists();
        assert!(l.remove_paths(["a.toml"]));
        assert!(l.get(1).is_some_and(|c| c.paths.is_empty()));
        assert!(!l.remove_paths(["nothing"]));
    }

    #[test]
    fn new_paths_join_the_active_list_only() {
        let mut l = lists();
        assert!(!l.capture_new_paths(["new.rs".to_string()]));
        l.set_active(Some(2));
        assert!(l.capture_new_paths(["new.rs".to_string(), "a.toml".to_string()]));
        assert_eq!(l.group_of("new.rs"), 2);
        assert_eq!(l.group_of("a.toml"), 1);
        l.delete(2);
        assert_eq!(l.active, None);
    }

    #[test]
    fn the_active_list_filters_the_commit() {
        let mut l = lists();
        assert!(l.commits_path("README"));
        l.set_active(Some(1));
        assert!(l.commits_path("a.toml"));
        assert!(!l.commits_path("README"));
        l.set_active(Some(99));
        assert_eq!(l.active, None);
    }

    #[test]
    fn a_restored_stash_brings_a_deleted_list_back() {
        let mut l = lists();
        let snapshot = l.snapshot(1).unwrap();
        l.note_stashed("abc", vec![snapshot]);
        assert_eq!(stash_label(&l, "abc").as_deref(), Some("Stash of Config"));
        l.delete(1);
        assert!(l.note_fate("abc", StashFate::Restored));
        let back = l.get(1).expect("re-created with its old id");
        assert_eq!(back.name, "Config");
        assert_eq!(back.description, "Local settings");
        assert!(back.paths.contains("a.toml"));
        assert!(stash_label(&l, "abc").is_none());
        assert!(!l.note_fate("abc", StashFate::Restored));
    }

    #[test]
    fn an_applied_stash_keeps_its_record() {
        let mut l = lists();
        let snapshot = l.snapshot(1).unwrap();
        l.note_stashed("abc", vec![snapshot]);
        l.delete(1);
        assert!(l.note_fate("abc", StashFate::Applied));
        assert!(l.get(1).is_some());
        assert_eq!(stash_label(&l, "abc").as_deref(), Some("Stash of Config"));
        // still there: nothing to revive the second time
        assert!(!l.note_fate("abc", StashFate::Applied));
    }

    #[test]
    fn a_dropped_stash_does_not_bring_the_list_back() {
        let mut l = lists();
        let snapshot = l.snapshot(1).unwrap();
        l.note_stashed("abc", vec![snapshot]);
        l.delete(1);
        assert!(l.note_fate("abc", StashFate::Dropped));
        assert!(l.get(1).is_none());
    }

    #[test]
    fn names_are_compared_ignoring_case() {
        let l = lists();
        assert!(l.name_taken("config", None));
        assert!(!l.name_taken("config", Some(1)));
        assert!(!l.name_taken("Other", None));
    }

    #[test]
    fn serialized_form_round_trips() {
        let mut l = lists();
        l.set_active(Some(2));
        l.set_collapsed(UNGROUPED, true);
        let json = serde_json::to_string(&l).unwrap();
        let back: Changelists = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
    }
}
