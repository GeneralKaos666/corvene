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

use std::path::PathBuf;

use corvene_git::{StashPop, StashPopOptions};
use corvene_models::{StashEntry, WorkingDirectoryFileChange};
use gpui_kit::App;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::{Popup, RepositoryState};

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
    /// The [`StashPopOptions`] the flags ask for.
    pub(crate) fn stash_pop_options(cx: &App) -> StashPopOptions {
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
        cx: &mut App,
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
        cx: &mut App,
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
    pub fn mark_stash_conflicts_resolved(id: u64, paths: Vec<String>, cx: &mut App) {
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
    pub fn open_stash_conflict_in_merge_tool(id: u64, file: String, cx: &mut App) {
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
    fn ask_drop_kept_stash(id: u64, workdir: PathBuf, resolved: Vec<String>, cx: &mut App) {
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

    /// `DropKeptStash` › Drop Stash: drop the entry whose commit is `sha`.
    pub fn drop_stash_entry(id: u64, sha: String, cx: &mut App) {
        Self::run_history_op(
            id,
            "Could not discard stash",
            move |git, workdir| corvene_git::drop_desktop_stash_entry(git, &workdir, &sha),
            cx,
        );
    }
}
