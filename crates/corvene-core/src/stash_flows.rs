//! Corvene's stash flows beyond GHD's (`app-store.ts` `_popStashEntry`,
//! `_dropStashEntry`, `checkoutAndBringChanges`).
//!
//! Deviation (`774-stash-conflict-flow`): a restore that conflicts keeps the
//! stash entry git kept and records it ([`KeptStash`]); the Changes view lists
//! the conflicted files (`corvene_ui::stash_conflicts`), Mark as Resolved
//! unstages a file's unmerged entries, and once none are left Corvene asks
//! whether to drop the kept entry (`Popup::DropKeptStash`). GHD drops the
//! entry after any pop git reports with exit code 1 and an empty stderr, and
//! has no way to mark such files resolved.
//!
//! Deviation (`775-stash-restore-unstages-new-files`): every restore passes
//! [`StashPopOptions::unstage_new_files`], so files that were untracked when
//! stashed come back untracked (GHD's come back staged as new files).
//!
//! Deviation (`776-stash-add-to-existing`): Overwrite Stash and "Unable to
//! … when changes are present" offer Add to Stash, which folds the changes
//! into the branch's stash when the two do not conflict
//! (`corvene_git::add_to_desktop_stash`); GHD can only overwrite the stash.
//!
//! Deviation (`777-stash-selected-files`): the changes list's file menu
//! stashes the selected files ([`Dispatcher::stash_selected_files`]) while
//! the branch has no stash of its own; GHD stashes all changes or none.
//!
//! Deviation (`778-overwritten-discard-and-continue`): "Unable to … when
//! changes are present" can discard the files it lists and run the
//! operation again ([`Dispatcher::discard_and_retry`]); GHD offers stashing
//! only.
//!
//! Deviation (`1204-restore-stash-from-other-branch`): another branch's
//! stash can be restored onto the current branch
//! ([`Dispatcher::restore_stash_from_branch`]); GHD restores a stash only on
//! the branch it was made on.
//!
//! Deviation (`283-move-changes-to-worktree`): the changes can be moved to
//! another worktree of the repository
//! ([`Dispatcher::move_changes_to_worktree`]); GHD 3.6's worktree switch
//! leaves them where they are.
//!
//! Deviation (`1207-switch-warns-target-behind`): the Switch Branch dialog
//! warns when the branch is behind its upstream (read by
//! [`Dispatcher::load_switch_target_behind`]); GHD does not look.

use std::path::PathBuf;

use crate::host::Host;
use corvene_git::{StashPop, StashPopOptions};
use corvene_models::{StashEntry, WorkingDirectoryFileChange};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup, RepositoryState};

/// `774-stash-conflict-flow`: a stash entry git kept because restoring it
/// conflicted, where (the worktree) and with which files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptStash {
    pub entry: StashEntry,
    pub workdir: PathBuf,
    pub files: Vec<String>,
}

impl RepositoryState {
    /// The conflicted files no merge, rebase or cherry-pick explains: those
    /// a stash restore (or another `git stash apply`) left (`774`).
    pub fn stash_conflicted_files(&self) -> Vec<&WorkingDirectoryFileChange> {
        if self.conflict_state.is_some() {
            return Vec::new();
        }
        self.status
            .as_deref()
            .map(|st| {
                st.files
                    .iter()
                    .filter(|f| f.status.is_conflicted())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// [`Self::kept_stash`] when it was recorded for `workdir`.
    pub fn kept_stash_for(&self, workdir: &std::path::Path) -> Option<&KeptStash> {
        self.kept_stash.as_ref().filter(|k| k.workdir == workdir)
    }
}

impl Dispatcher {
    /// `776-stash-add-to-existing`: Add to Stash is offered (the flag is on
    /// and git is new enough for `merge-tree --merge-base`).
    pub fn add_to_stash_available(s: &AppState) -> bool {
        s.flags.bool(crate::flags::ids::STASH_ADD_TO_EXISTING)
            && s.git.as_ref().is_some_and(|git| {
                (git.version.major, git.version.minor) >= corvene_git::ADD_TO_STASH_MIN_VERSION
            })
    }

    /// The [`StashPopOptions`] the flags ask for.
    pub(crate) fn stash_pop_options(cx: &dyn Host) -> StashPopOptions {
        let flags = &Self::state(cx).read(cx).flags;
        StashPopOptions {
            keep_on_conflict: flags.bool(crate::flags::ids::STASH_CONFLICT_FLOW),
            unstage_new_files: flags.bool(crate::flags::ids::STASH_RESTORE_UNSTAGES_NEW_FILES),
        }
    }

    /// After a restore in `workdir`: a conflicted one whose entry git kept is
    /// recorded, with the files then unmerged (`774`).
    pub(crate) fn note_stash_pop(
        id: u64,
        workdir: PathBuf,
        kept: Option<(StashEntry, Vec<String>)>,
        cx: &mut dyn Host,
    ) {
        let Some((entry, files)) = kept else {
            return;
        };
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::STASH_CONFLICT_FLOW)
        {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).kept_stash = Some(KeptStash {
                entry,
                workdir,
                files,
            });
            cx.notify();
        });
    }

    /// The entry and unmerged files to record when `pop` (of `entry`, in
    /// `workdir`) conflicted; runs git, so call it in the background.
    pub(crate) fn kept_after_pop(
        git: std::sync::Arc<corvene_git::GitBinary>,
        workdir: &std::path::Path,
        entry: &StashEntry,
        pop: StashPop,
    ) -> Option<(StashEntry, Vec<String>)> {
        (pop == StashPop::Conflicted).then(|| {
            let files = corvene_git::unmerged_paths(git, workdir).unwrap_or_default();
            (entry.clone(), files)
        })
    }

    /// Stashed Changes › Restore: the pop of [`Self::pop_stash`] with the
    /// flags' [`StashPopOptions`], recording a conflicted restore (`774`).
    pub(crate) fn pop_stash_with_options(
        id: u64,
        stash: StashEntry,
        check_branch: bool,
        cx: &mut dyn Host,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let options = Self::stash_pop_options(cx);
        let path = workdir.clone();
        spawn_bg(
            cx,
            move || {
                let pop = match (check_branch, stash.branch.as_deref()) {
                    (true, Some(branch)) => corvene_git::pop_stash_on_branch(
                        git.clone(),
                        &workdir,
                        &stash.sha,
                        branch,
                        options,
                    )?,
                    _ => corvene_git::pop_stash_entry_with(
                        git.clone(),
                        &workdir,
                        &stash.sha,
                        options,
                    )?,
                };
                Ok::<_, corvene_git::GitError>(Self::kept_after_pop(git, &workdir, &stash, pop))
            },
            move |result, cx| {
                match result {
                    Ok(kept) => Self::note_stash_pop(id, path, kept, cx),
                    Err(err) => Self::show_error("Could not restore stash", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `774` › Mark as Resolved: unstage the unmerged entries of `paths`
    /// (the files stay as they are), then ask about the kept stash once no
    /// conflicted file is left.
    pub fn mark_stash_conflicts_resolved(id: u64, paths: Vec<String>, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let path = workdir.clone();
        let marked = paths.clone();
        spawn_bg(
            cx,
            move || {
                corvene_git::mark_conflicts_resolved(git.clone(), &workdir, &paths)?;
                corvene_git::unmerged_paths(git, &workdir)
            },
            move |result, cx| {
                match result {
                    Ok(left) if left.is_empty() => Self::ask_drop_kept_stash(id, path, marked, cx),
                    Ok(_) => {}
                    Err(err) => Self::show_error("Could not mark the file as resolved", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `774`: `git mergetool` on a file a restore left conflicted (it stages
    /// the file when the tool reports success), then as
    /// [`Self::mark_stash_conflicts_resolved`] once none is left.
    pub fn open_stash_conflict_in_merge_tool(id: u64, file: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let path = workdir.clone();
        let marked = vec![file.clone()];
        spawn_bg(
            cx,
            move || {
                corvene_git::open_merge_tool(git.clone(), &workdir, &file)?;
                corvene_git::unmerged_paths(git, &workdir)
            },
            move |result, cx| {
                match result {
                    Ok(left) if left.is_empty() => Self::ask_drop_kept_stash(id, path, marked, cx),
                    Ok(_) => {}
                    Err(err) => Self::show_error("Could not open the merge tool", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// No conflicted file is left in `workdir` after resolving `resolved`:
    /// when the kept stash recorded there conflicted in one of them and
    /// still exists, ask whether to drop it (the record is used up).
    fn ask_drop_kept_stash(id: u64, workdir: PathBuf, resolved: Vec<String>, cx: &mut dyn Host) {
        let kept = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            let matches = rs.kept_stash_for(&workdir).is_some_and(|k| {
                k.files.is_empty() || resolved.iter().any(|p| k.files.contains(p))
            });
            if matches { rs.kept_stash.take() } else { None }
        });
        let Some(kept) = kept else {
            return;
        };
        let Some(git) = Self::state(cx).read(cx).git.clone() else {
            return;
        };
        let sha = kept.entry.sha.clone();
        spawn_bg(
            cx,
            move || {
                corvene_git::stash_entry(git, &kept.workdir, &sha)
                    .ok()
                    .flatten()
            },
            move |entry, cx| {
                if let Some(stash) = entry {
                    Self::show_popup(Popup::DropKeptStash { repo: id, stash }, cx);
                }
            },
        );
    }

    /// `776-stash-add-to-existing` › Add to Stash: fold the changes into the
    /// current branch's stash; `then(true)` once they are stashed (also when
    /// there was nothing to add), `then(false)` when nothing changed.
    pub(crate) fn add_to_stash_then(
        id: u64,
        then: impl FnOnce(bool, &mut dyn Host) + 'static,
        cx: &mut dyn Host,
    ) {
        if !Self::add_to_stash_available(Self::state(cx).read(cx)) {
            then(false, cx);
            return;
        }
        let (repo, branch, old, guard) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            (
                Self::repo_context(id, cx),
                rs.and_then(|r| r.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                rs.and_then(|r| r.desktop_stash()).map(|e| e.sha.clone()),
                s.flags
                    .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED),
            )
        };
        let (Some((git, workdir)), Some(branch), Some(old)) = (repo, branch, old) else {
            then(false, cx);
            return;
        };
        spawn_bg(
            cx,
            move || corvene_git::add_to_desktop_stash(git, &workdir, &old, &branch, guard),
            move |result, cx| {
                let ok = match result {
                    Ok(corvene_git::AddToStash::Added | corvene_git::AddToStash::NothingToAdd) => {
                        true
                    }
                    Ok(corvene_git::AddToStash::Conflicts(files)) => {
                        Self::show_error(
                            "Could not add to the stash",
                            add_to_stash_conflict_message(&files),
                            cx,
                        );
                        false
                    }
                    Err(err) => {
                        Self::show_error("Could not add to the stash", &err, cx);
                        false
                    }
                };
                Self::refresh_repository(id, cx);
                then(ok, cx);
            },
        );
    }

    /// Branch › Stash All Changes › Add to Stash (`776`).
    pub fn add_to_stash(id: u64, cx: &mut dyn Host) {
        Self::add_to_stash_then(id, |_, _| {}, cx);
    }

    /// Switch Branch › Overwrite Stash › Add to Stash (`776`): the changes
    /// join the stash, then `branch` is checked out.
    pub fn add_to_stash_and_checkout(id: u64, branch: String, cx: &mut dyn Host) {
        Self::add_to_stash_then(
            id,
            move |ok, cx| {
                if ok {
                    Self::checkout_branch(
                        id,
                        branch,
                        Some(crate::persistence::UncommittedChangesStrategy::StashOnCurrentBranch),
                        cx,
                    );
                }
            },
            cx,
        );
    }

    /// "Unable to … when changes are present" › Add to Stash and Continue
    /// (`776`): the changes join the stash, then the operation runs again.
    pub fn add_to_stash_and_retry(id: u64, retry: crate::state::RetryAction, cx: &mut dyn Host) {
        Self::close_popup(cx);
        Self::add_to_stash_then(
            id,
            move |ok, cx| {
                if ok {
                    Self::perform_retry(id, retry, cx);
                } else {
                    Self::end_mco(id, cx);
                }
            },
            cx,
        );
    }

    /// `777-stash-selected-files` › Stash N Selected Files: a Desktop stash
    /// of `paths` only, on the current branch. Refused while the branch has
    /// a stash (the menu item is disabled then).
    pub fn stash_selected_files(id: u64, paths: Vec<String>, cx: &mut dyn Host) {
        let (branch, files, has_stash, guard) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let wanted: std::collections::HashSet<&str> =
                paths.iter().map(String::as_str).collect();
            (
                rs.info
                    .as_ref()
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                rs.status
                    .as_deref()
                    .map(|st| {
                        st.files
                            .iter()
                            .filter(|f| wanted.contains(f.path.as_str()))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                rs.desktop_stash().is_some(),
                s.flags
                    .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED),
            )
        };
        let Some(branch) = branch else {
            return;
        };
        if has_stash || files.is_empty() {
            return;
        }
        Self::run_history_op(
            id,
            "Could not stash changes",
            move |git, workdir| {
                corvene_git::create_desktop_stash_of_files(git, &workdir, &branch, &files, guard)
                    .map(|_| ())
            },
            cx,
        );
    }

    /// `778-overwritten-discard-and-continue` › Discard Changes and
    /// Continue: discard `paths` the usual way (new files to the Trash),
    /// then run the operation again. A failed discard, or files the Trash
    /// refused, stop before the retry.
    pub fn discard_and_retry(
        id: u64,
        paths: Vec<String>,
        retry: crate::state::RetryAction,
        cx: &mut dyn Host,
    ) {
        Self::close_popup(cx);
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (files, clean_submodules, move_to_trash, keep_untrashable) = {
            let s = Self::state(cx).read(cx);
            let wanted: std::collections::HashSet<&str> =
                paths.iter().map(String::as_str).collect();
            (
                s.repo_states
                    .get(&id)
                    .and_then(|r| r.status.as_deref())
                    .map(|st| {
                        st.files
                            .iter()
                            .filter(|f| wanted.contains(f.path.as_str()))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                s.flags.bool(crate::flags::ids::DISCARD_SUBMODULE_CHANGES),
                !s.flags.bool(crate::flags::ids::DISCARD_SKIPS_TRASH),
                // GHD `askForConfirmationOnDiscardChangesPermanently`
                s.settings.confirm_discard_changes_permanently,
            )
        };
        spawn_bg(
            cx,
            move || {
                if files.is_empty() {
                    return Ok(Vec::new());
                }
                corvene_git::discard_changes(
                    git,
                    &workdir,
                    &files,
                    move_to_trash,
                    clean_submodules,
                    keep_untrashable,
                )
            },
            move |result, cx| match result {
                Ok(untrashable) if untrashable.is_empty() => Self::perform_retry(id, retry, cx),
                // `DiscardChangesRetry`: files are left, so no retry yet
                Ok(untrashable) => {
                    Self::end_mco(id, cx);
                    Self::show_popup(
                        Popup::ConfirmDeleteUntrashable {
                            repo: id,
                            paths: untrashable,
                        },
                        cx,
                    );
                    Self::refresh_repository(id, cx);
                }
                Err(err) => {
                    Self::end_mco(id, cx);
                    Self::show_error("Could not discard changes", &err, cx);
                    Self::refresh_repository(id, cx);
                }
            },
        );
    }

    /// `1204-restore-stash-from-other-branch` › Restore Stash Here: pop the
    /// newest Desktop stash made on `branch` onto the current branch, as
    /// Restore does (only with no local changes; a conflicted restore keeps
    /// the entry under `774`).
    pub fn restore_stash_from_branch(id: u64, branch: String, cx: &mut dyn Host) {
        let has_changes = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.changed_files() > 0);
        if has_changes {
            Self::show_error(
                "Could not restore stash",
                "Commit, stash or discard your changes before restoring another branch's stash.",
                cx,
            );
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let options = Self::stash_pop_options(cx);
        let path = workdir.clone();
        spawn_bg(
            cx,
            move || {
                let Some(entry) = corvene_git::get_last_desktop_stash_entry_for_branch(
                    git.clone(),
                    &workdir,
                    &branch,
                )?
                else {
                    return Err(corvene_git::GitError::Gix(format!(
                        "\"{branch}\" has no stash any more; it may have been restored or discarded \
                         already."
                    )));
                };
                let pop =
                    corvene_git::pop_stash_entry_with(git.clone(), &workdir, &entry.sha, options)?;
                Ok(Self::kept_after_pop(git, &workdir, &entry, pop))
            },
            move |result, cx| {
                match result {
                    Ok(kept) => Self::note_stash_pop(id, path, kept, cx),
                    Err(err) => Self::show_error("Could not restore stash", &err, cx),
                }
                Self::show_section(id, corvene_models::Section::Changes, cx);
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `283-move-changes-to-worktree`: Move Changes to Worktree… (changes
    /// list menu, Branch menu).
    pub fn show_move_changes_to_worktree(id: u64, cx: &mut dyn Host) {
        let ready = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.changed_files() > 0 && r.worktrees.len() > 1);
        if ready {
            Self::show_popup(Popup::MoveChangesToWorktree { repo: id }, cx);
        }
    }

    /// `283-move-changes-to-worktree`: stash every change here (a Desktop
    /// stash named after the target's branch), restore it in the worktree
    /// at `target` (stashes are shared by a repository's worktrees), then
    /// switch to it when `switch`. Refused while `target` has changes of
    /// its own. A conflicted restore keeps the stash there (`774`); any other
    /// failure after stashing keeps it too, and says so.
    pub fn move_changes_to_worktree(id: u64, target: PathBuf, switch: bool, cx: &mut dyn Host) {
        let Some((git, source)) = Self::repo_context(id, cx) else {
            return;
        };
        let (label, guard) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let target_branch = rs
                .and_then(|r| r.worktrees.iter().find(|w| w.path == target))
                .and_then(|w| w.branch.as_deref())
                .map(|b| b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
            let current = rs
                .and_then(|r| r.info.as_ref())
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone());
            (
                target_branch.or(current).unwrap_or_else(|| "HEAD".into()),
                s.flags
                    .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED),
            )
        };
        let options = Self::stash_pop_options(cx);
        let there = target.clone();
        spawn_bg(
            cx,
            move || -> Result<Option<(StashPop, StashEntry, Vec<String>)>, corvene_git::GitError> {
                let name = there
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| there.display().to_string());
                if !corvene_git::get_status(git.clone(), &there)?.files.is_empty() {
                    return Err(corvene_git::GitError::Gix(format!(
                        "The worktree {name} has uncommitted changes. Commit, stash or discard \
                         them there first."
                    )));
                }
                if guard {
                    corvene_git::ensure_no_modified_assume_unchanged(git.clone(), &source)?;
                }
                if !corvene_git::create_desktop_stash(git.clone(), &source, &label, false)? {
                    return Ok(None);
                }
                let entry =
                    corvene_git::get_last_desktop_stash_entry_for_branch(git.clone(), &source, &label)?
                        .ok_or_else(|| {
                            corvene_git::GitError::Gix("The new stash entry could not be found.".into())
                        })?;
                let pop = corvene_git::pop_stash_entry_with(git.clone(), &there, &entry.sha, options)
                    .map_err(|err| {
                        corvene_git::GitError::Gix(format!(
                            "Your changes were stashed but could not be restored in {name}; they \
                             are kept in the stash list. {err}"
                        ))
                    })?;
                let files = if pop == StashPop::Conflicted {
                    corvene_git::unmerged_paths(git, &there).unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok(Some((pop, entry, files)))
            },
            move |result, cx| {
                match result {
                    Ok(Some((pop, entry, files))) => {
                        if pop == StashPop::Conflicted {
                            Self::note_stash_pop(id, target.clone(), Some((entry, files)), cx);
                        }
                        if switch {
                            Self::switch_worktree(id, target, cx);
                        }
                    }
                    Ok(None) => {}
                    Err(err) => Self::show_error("Could not move the changes", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `1207-switch-warns-target-behind`: count the commits of `branch`'s
    /// upstream (as of the last fetch) that `branch` lacks, for the Switch
    /// Branch dialog's warning.
    pub(crate) fn load_switch_target_behind(
        id: u64,
        branch: &corvene_models::Branch,
        cx: &mut dyn Host,
    ) {
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).switch_target_behind = None);
        if branch.kind != corvene_models::BranchKind::Local
            || !Self::state(cx)
                .read(cx)
                .flags
                .bool(crate::flags::ids::SWITCH_WARNS_TARGET_BEHIND)
        {
            return;
        }
        let Some(upstream) = branch.upstream_short().map(str::to_string) else {
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let name = branch.name.clone();
        spawn_bg(
            cx,
            {
                let (name, upstream) = (name.clone(), upstream.clone());
                move || corvene_git::commits_ahead(git, &workdir, &name, &upstream).ok()
            },
            move |behind, cx| {
                if let Some(behind) = behind.filter(|n| *n > 0) {
                    Self::state(cx).update(cx, |s, cx| {
                        s.repo_state_mut(id).switch_target_behind = Some((name, upstream, behind));
                        cx.notify();
                    });
                }
            },
        );
    }

    /// `DropKeptStash` › Drop Stash: drop the entry whose commit is `sha`.
    pub fn drop_stash_entry(id: u64, sha: String, cx: &mut dyn Host) {
        Self::run_history_op(
            id,
            "Could not discard stash",
            move |git, workdir| corvene_git::drop_desktop_stash_entry(git, &workdir, &sha),
            cx,
        );
    }
}

/// Why Add to Stash stopped: the files both change (at most five named).
pub fn add_to_stash_conflict_message(files: &[String]) -> String {
    let shown: Vec<&str> = files.iter().take(5).map(String::as_str).collect();
    let more = match files.len().saturating_sub(shown.len()) {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    let what = if shown.is_empty() {
        "the same files".to_string()
    } else {
        format!("{}{more}", shown.join(", "))
    };
    format!(
        "Your changes and the stash both change {what}, so they cannot be combined. Nothing \
         was changed: your changes and the stash are as they were. Restore or discard the \
         stash first, or overwrite it."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_to_stash_conflicts_name_the_files() {
        let files: Vec<String> = (1..=7).map(|i| format!("f{i}.txt")).collect();
        let message = add_to_stash_conflict_message(&files);
        assert!(
            message.contains("f1.txt, f2.txt, f3.txt, f4.txt, f5.txt and 2 more"),
            "{message}"
        );
        assert!(add_to_stash_conflict_message(&[]).contains("the same files"));
    }
}
