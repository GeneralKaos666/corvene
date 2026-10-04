//! The checks GHD's commit flow runs before committing (`onCreateCommit`,
//! `app/src/ui/changes/sidebar.tsx`): included files over 100 MiB
//! (`getLargeFilePaths`, `app/src/lib/large-files.ts`) that Git LFS does not
//! track open `OversizedFiles` (`app/src/ui/changes/oversized-files-warning.tsx`)
//! instead of committing; its "Commit Anyway" commits them.
//!
//! GHD runs them in the view after the commit message's own checks
//! (unknown co-authors); Corvene runs them in [`Dispatcher::commit_with`], which
//! the commit form and those dialogs call, off the main thread.

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::state::Popup;
use corvene_models::DiffSelectionType;

/// What the user already agreed to in this commit's dialogs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitChecks {
    /// `OversizedFiles` › Commit Anyway.
    pub allow_oversized: bool,
}

impl Dispatcher {
    /// Commit the included changes, after the checks `checks` does not
    /// settle yet.
    pub fn commit_with(
        id: u64,
        summary: String,
        description: String,
        checks: CommitChecks,
        cx: &mut dyn Host,
    ) {
        if checks.allow_oversized {
            return Self::create_commit(id, summary, description, cx);
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (paths, committing) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let paths: Vec<String> = rs
                .and_then(|rs| rs.status.as_deref())
                .map(|st| {
                    st.files
                        .iter()
                        .filter(|f| f.selection.kind() != DiffSelectionType::None)
                        .map(|f| f.path.clone())
                        .collect()
                })
                .unwrap_or_default();
            (paths, rs.is_some_and(|rs| rs.committing))
        };
        if committing {
            return;
        }
        if paths.is_empty() {
            return Self::create_commit(id, summary, description, cx);
        }
        Self::set_committing(id, true, cx);
        crate::remote::spawn_bg(
            cx,
            move || {
                let large =
                    corvene_git::large_file_paths(&workdir, &paths, corvene_git::RECEIVE_LIMIT);
                if large.is_empty() {
                    return large;
                }
                corvene_git::files_not_tracked_by_lfs(git, &workdir, &large).unwrap_or(large)
            },
            move |oversized, cx| {
                Self::set_committing(id, false, cx);
                if oversized.is_empty() {
                    let checks = CommitChecks {
                        allow_oversized: true,
                    };
                    Self::commit_with(id, summary, description, checks, cx);
                } else {
                    Self::show_popup(
                        Popup::OversizedFiles {
                            repo: id,
                            files: oversized,
                            summary,
                            description,
                        },
                        cx,
                    );
                }
            },
        );
    }

    fn set_committing(id: u64, committing: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).committing = committing;
            cx.notify();
        });
    }
}
