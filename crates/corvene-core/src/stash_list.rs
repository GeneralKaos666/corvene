//! Corvene's stash list (`797-stash-list`), beyond GHD's single Desktop
//! stash per branch (`app-store.ts` `_popStashEntry`, `_dropStashEntry`).
//!
//! Deviation (`797-stash-list`): every entry of `refs/stash` is listed
//! ([`RepositoryState::stashes`]), command-line stashes included; one can be
//! shown in the stash viewer ([`Dispatcher::view_stash`]), restored, applied
//! without dropping it, turned into a branch (`git stash branch`) or
//! discarded with an Undo banner ([`Dispatcher::discard_stash_entry`]).
//! Stash All Changes with Message ([`Dispatcher::stash_with_message`])
//! makes a stash without Desktop's marker, so branch switches leave it
//! alone. GHD shows and acts on the current branch's
//! `!!GitHub_Desktop<branch>` stash only, and drops a discarded one for good.

use corvene_models::StashEntry;

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::mco::Banner;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// What the stash list calls `entry`: "Stashed changes" for a Desktop
/// stash, else the message git or the user gave it without git's
/// `On <branch>: ` prefix (`WIP on <branch>: <sha> <subject>` becomes
/// `WIP: <subject>`).
pub fn stash_title(entry: &StashEntry) -> String {
    if entry.branch.is_some() {
        return "Stashed changes".to_string();
    }
    message_title(&entry.message).unwrap_or_else(|| entry.name.clone())
}

/// [`stash_title`] from a stash's message alone (`None` when it is empty).
pub fn message_title(message: &str) -> Option<String> {
    let message = message.trim();
    if message.contains(corvene_git::DESKTOP_STASH_MARKER) {
        return Some("Stashed changes".to_string());
    }
    if let Some(rest) = message.strip_prefix("WIP on ")
        && let Some((_, wip)) = rest.split_once(": ")
    {
        let subject = wip
            .split_once(' ')
            .map_or("", |(_, subject)| subject)
            .trim();
        return Some(if subject.is_empty() {
            "WIP".to_string()
        } else {
            format!("WIP: {subject}")
        });
    }
    if let Some(rest) = message.strip_prefix("On ")
        && let Some((_, text)) = rest.split_once(": ")
        && !text.trim().is_empty()
    {
        return Some(text.trim().to_string());
    }
    (!message.is_empty()).then(|| message.to_string())
}

/// The branch `entry` was made on: the one in a Desktop stash's marker, else
/// the one in git's `On <branch>: ` / `WIP on <branch>: ` prefix (`None` for
/// a detached `HEAD` or a message without either).
pub fn stash_branch(entry: &StashEntry) -> Option<String> {
    if let Some(branch) = &entry.branch {
        return Some(branch.clone());
    }
    let message = entry.message.trim();
    let rest = message
        .strip_prefix("WIP on ")
        .or_else(|| message.strip_prefix("On "))?;
    let (branch, _) = rest.split_once(": ")?;
    (branch != "(no branch)" && !branch.is_empty()).then(|| branch.to_string())
}

impl Dispatcher {
    fn stash_list_entry(id: u64, sha: &str, cx: &dyn Host) -> Option<StashEntry> {
        Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)?
            .stashes
            .iter()
            .find(|s| s.sha == sha)
            .cloned()
    }

    /// A stash list row: show entry `sha` in the stash viewer.
    pub fn view_stash(id: u64, sha: String, cx: &mut dyn Host) {
        let load = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if !rs.stashes.iter().any(|e| e.sha == sha) {
                return false;
            }
            if rs.shown_stash().map(|e| &e.sha) != Some(&sha) {
                rs.stash_files = None;
                rs.stash_files_sha = None;
                rs.stash_diff = None;
                rs.stash_selected_file = None;
            }
            rs.viewed_stash = Some(sha);
            rs.showing_stash = true;
            cx.notify();
            rs.stash_files.is_none()
        });
        if load {
            Self::load_stash_files(id, cx);
        }
    }

    /// Restore: `git stash pop` of entry `sha`, onto the checked-out branch.
    pub fn restore_stash_entry(id: u64, sha: String, cx: &mut dyn Host) {
        let Some(stash) = Self::stash_list_entry(id, &sha, cx) else {
            return;
        };
        Self::pop_stash_with_options(id, stash, false, cx);
    }

    /// Apply: `git stash apply` of entry `sha`; the entry stays. Conflicts
    /// show in the Changes list (`774-stash-conflict-flow`), with no prompt
    /// to drop the entry afterwards.
    pub fn apply_stash_entry(id: u64, sha: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let options = Self::stash_pop_options(cx);
        let applied = sha.clone();
        spawn_bg(
            cx,
            move || corvene_git::apply_stash_entry_with(git, &workdir, &sha, options),
            move |result, cx| {
                match result {
                    Ok(corvene_git::StashPop::Missing) => {}
                    Ok(_) => Self::note_changelist_stash_fate(
                        id,
                        &applied,
                        crate::changelists::StashFate::Applied,
                        cx,
                    ),
                    Err(err) => Self::show_error("Could not apply stash", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// Discard (stash list): confirm unless the user opted out
    /// (`askForConfirmationOnDiscardStash`).
    pub fn request_discard_stash_entry(id: u64, sha: String, cx: &mut dyn Host) {
        let Some(stash) = Self::stash_list_entry(id, &sha, cx) else {
            return;
        };
        if Self::state(cx).read(cx).settings.confirm_discard_stash {
            Self::show_popup(Popup::ConfirmDropStashEntry { repo: id, stash }, cx);
        } else {
            Self::discard_stash_entry(id, stash, cx);
        }
    }

    /// `git stash drop` of `stash`, then a "Discarded stash" banner whose
    /// Undo stores it again ([`Self::restore_discarded_stash`]).
    pub fn discard_stash_entry(id: u64, stash: StashEntry, cx: &mut dyn Host) {
        let sha = stash.sha.clone();
        Self::run_history_op_then(
            id,
            "Could not discard stash",
            move |git, workdir| corvene_git::drop_desktop_stash_entry(git, &workdir, &sha),
            move |cx| {
                Self::note_changelist_stash_fate(
                    id,
                    &stash.sha,
                    crate::changelists::StashFate::Dropped,
                    cx,
                );
                Self::set_banner(
                    Banner::StashDropped {
                        repo: id,
                        sha: stash.sha,
                        message: stash.message,
                    },
                    cx,
                )
            },
            cx,
        );
    }

    /// The "Discarded stash" banner's Undo: `git stash store` the commit
    /// again with its message (it is newest in the list afterwards).
    pub fn restore_discarded_stash(id: u64, sha: String, message: String, cx: &mut dyn Host) {
        Self::run_history_op_then(
            id,
            "Could not restore stash",
            move |git, workdir| corvene_git::store_stash(git, &workdir, &sha, &message),
            |cx| Self::set_banner(Banner::StashRestored, cx),
            cx,
        );
    }

    /// Stash All Changes with Message…
    pub fn request_stash_with_message(id: u64, cx: &mut dyn Host) {
        Self::show_popup(Popup::StashWithMessage { repo: id }, cx);
    }

    /// `git stash push [--include-untracked] [-m <message>]` of every change.
    pub fn stash_with_message(
        id: u64,
        message: String,
        include_untracked: bool,
        cx: &mut dyn Host,
    ) {
        let guard = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED);
        Self::run_history_op(
            id,
            "Could not stash changes",
            move |git, workdir| {
                corvene_git::create_stash_with_message(
                    git,
                    &workdir,
                    Some(&message),
                    include_untracked,
                    guard,
                )
                .map(|_| ())
            },
            cx,
        );
    }

    /// Create Branch from Stash…
    pub fn request_create_branch_from_stash(id: u64, sha: String, cx: &mut dyn Host) {
        let Some(stash) = Self::stash_list_entry(id, &sha, cx) else {
            return;
        };
        Self::show_popup(Popup::CreateBranchFromStash { repo: id, stash }, cx);
    }

    /// `git stash branch <branch>` of `stash`: the branch starts at the
    /// commit the stash was made on and is checked out with the changes; a
    /// conflicted apply keeps the entry (`774-stash-conflict-flow`).
    pub fn create_branch_from_stash(id: u64, stash: StashEntry, branch: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let path = workdir.clone();
        spawn_bg(
            cx,
            move || {
                let made = corvene_git::create_branch_from_stash(
                    git.clone(),
                    &workdir,
                    &stash.sha,
                    &branch,
                )?;
                Ok::<_, corvene_git::GitError>(Self::kept_after_pop(git, &workdir, &stash, made))
            },
            move |result, cx| {
                match result {
                    Ok(kept) => Self::note_stash_pop(id, path, kept, cx),
                    Err(err) => Self::show_error("Could not create branch from stash", &err, cx),
                }
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.viewed_stash = None;
                    rs.showing_stash = false;
                    cx.notify();
                });
                Self::refresh_repository(id, cx);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(message: &str, branch: Option<&str>) -> StashEntry {
        StashEntry {
            name: "stash@{0}".into(),
            sha: "abc".into(),
            branch: branch.map(Into::into),
            message: message.into(),
            tree: String::new(),
            parents: Vec::new(),
            date: 0,
        }
    }

    #[test]
    fn titles_drop_gits_prefix() {
        let desktop = entry("On main: !!GitHub_Desktop<main>", Some("main"));
        assert_eq!(stash_title(&desktop), "Stashed changes");
        assert_eq!(stash_branch(&desktop).as_deref(), Some("main"));
        let named = entry("On feature/x: try the other parser", None);
        assert_eq!(stash_title(&named), "try the other parser");
        assert_eq!(stash_branch(&named).as_deref(), Some("feature/x"));
        let wip = entry("WIP on main: 1a2b3c4 Fix the thing", None);
        assert_eq!(stash_title(&wip), "WIP: Fix the thing");
        assert_eq!(stash_branch(&wip).as_deref(), Some("main"));
        let detached = entry("WIP on (no branch): 1a2b3c4 Detached", None);
        assert_eq!(stash_branch(&detached), None);
        let autostash = entry("autostash", None);
        assert_eq!(stash_title(&autostash), "autostash");
        assert_eq!(stash_branch(&autostash), None);
        assert_eq!(stash_title(&entry("", None)), "stash@{0}");
    }
}
