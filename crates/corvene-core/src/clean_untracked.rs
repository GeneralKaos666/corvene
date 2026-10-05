//! Corvene (`1105-clean-untracked-files`): Repository › Clean Untracked
//! Files… and the changes list's menu item of the same name open a dialog
//! (`Popup::CleanUntrackedFiles`) listing what `git clean -n -d` would
//! remove, a checkbox each, with an option to include ignored files (`-x`);
//! the ticked paths go with `git clean -f -d`. GHD has no `git clean`: it
//! discards untracked files one by one (to the Trash) and never touches
//! ignored files.

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// The dry run the dialog shows (`RepositoryState::clean_preview`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanPreview {
    /// Run with `-x`.
    pub include_ignored: bool,
    pub loading: bool,
    /// `git clean -n` paths; a folder ends with `/`.
    pub paths: Vec<String>,
    pub error: Option<String>,
}

impl Dispatcher {
    /// Repository › Clean Untracked Files…: the dialog, and its first dry
    /// run (ignored files left out).
    pub fn show_clean_untracked_files(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::CLEAN_UNTRACKED_FILES)
        {
            return;
        }
        Self::show_popup(Popup::CleanUntrackedFiles { repo: id }, cx);
        Self::load_clean_preview(id, false, cx);
    }

    /// `git clean -n -d [-x]` into `RepositoryState::clean_preview`. A
    /// result for the other `include_ignored` (the box was toggled again
    /// meanwhile) is dropped.
    pub fn load_clean_preview(id: u64, include_ignored: bool, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let previous = rs.clean_preview.take().unwrap_or_default();
            rs.clean_preview = Some(CleanPreview {
                include_ignored,
                loading: true,
                // the old list stays until the new one is in (no flicker)
                paths: previous.paths,
                error: None,
            });
            cx.notify();
        });
        spawn_bg(
            cx,
            move || corvene_git::clean_dry_run(git, &workdir, include_ignored),
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    let Some(preview) = rs.clean_preview.as_mut() else {
                        return;
                    };
                    if preview.include_ignored != include_ignored {
                        return;
                    }
                    preview.loading = false;
                    match result {
                        Ok(paths) => preview.paths = paths,
                        Err(err) => {
                            preview.paths.clear();
                            preview.error = Some(err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Clean Untracked Files › Remove: `git clean -f -d [-x]` of `paths`
    /// (from the dry run). They are deleted for good.
    pub fn clean_untracked_files(
        id: u64,
        paths: Vec<String>,
        include_ignored: bool,
        cx: &mut dyn Host,
    ) {
        Self::close_popup(cx);
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).clean_preview = None);
        if paths.is_empty() {
            return;
        }
        Self::run_history_op(
            id,
            "Could not remove the files",
            move |git, workdir| corvene_git::clean_paths(git, &workdir, &paths, include_ignored),
            cx,
        );
    }

    /// The dialog closed without removing anything.
    pub fn close_clean_untracked_files(id: u64, cx: &mut dyn Host) {
        Self::close_popup(cx);
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).clean_preview = None);
    }
}
