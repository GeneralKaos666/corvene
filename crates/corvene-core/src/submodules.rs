//! Corvene (`1111-submodules`): Repository › Submodules… opens a panel
//! (`Popup::Submodules`) listing every submodule (`corvene_git::
//! submodule_details`: path, URL, recorded and checked-out commit, state)
//! with Initialize, Update (`--init`, optionally `--recursive`) and Sync for
//! one submodule or all of them, and Open as Repository
//! ([`Dispatcher::open_submodule`], as the changes list's
//! `284-open-submodule-from-changes` item). GHD has no such panel: it only
//! runs `git submodule update --init --recursive` after a checkout
//! (`lib/git/submodule.ts`).

use std::path::PathBuf;

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// A panel action; `paths` empty means every submodule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmoduleAction {
    Init,
    Update,
    Sync,
}

/// What the panel shows (`RepositoryState::submodules`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmodulesState {
    pub loading: bool,
    pub list: Vec<corvene_git::SubmoduleDetails>,
    /// The last action's (or listing's) failure, shown in the panel.
    pub error: Option<String>,
    /// The running action ("Updating vendor/lib…").
    pub busy: Option<String>,
    /// Update and Sync pass `--recursive`.
    pub recursive: bool,
}

impl Default for SubmodulesState {
    fn default() -> Self {
        Self {
            loading: true,
            list: Vec::new(),
            error: None,
            busy: None,
            recursive: true,
        }
    }
}

impl Dispatcher {
    /// Repository › Submodules….
    pub fn show_submodules(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::SUBMODULES)
        {
            return;
        }
        Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            if rs.submodules.is_none() {
                rs.submodules = Some(SubmodulesState::default());
            }
        });
        Self::show_popup(Popup::Submodules { repo: id }, cx);
        Self::load_submodules(id, cx);
    }

    /// `corvene_git::submodule_details` into `RepositoryState::submodules`
    /// (the old list stays until the new one is in).
    pub fn load_submodules(id: u64, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            if let Some(state) = s.repo_state_mut(id).submodules.as_mut() {
                state.loading = true;
                cx.notify();
            }
        });
        spawn_bg(
            cx,
            move || corvene_git::submodule_details(git, &workdir),
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(state) = s.repo_state_mut(id).submodules.as_mut() else {
                        return;
                    };
                    state.loading = false;
                    match result {
                        Ok(list) => state.list = list,
                        Err(err) => {
                            state.list.clear();
                            state.error = Some(err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }

    /// The panel's Recursive checkbox.
    pub fn set_submodules_recursive(id: u64, recursive: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(state) = s.repo_state_mut(id).submodules.as_mut() {
                state.recursive = recursive;
                cx.notify();
            }
        });
    }

    /// Initialize / Update / Sync `paths` (every submodule for none), then
    /// list them again and refresh the repository.
    pub fn run_submodule_action(
        id: u64,
        action: SubmoduleAction,
        paths: Vec<String>,
        cx: &mut dyn Host,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let recursive = Self::state(cx).update(cx, |s, cx| {
            let state = s.repo_state_mut(id).submodules.get_or_insert_default();
            let target = match paths.as_slice() {
                [one] => one.clone(),
                _ => "all submodules".to_string(),
            };
            state.busy = Some(match action {
                SubmoduleAction::Init => format!("Initializing {target}…"),
                SubmoduleAction::Update => format!("Updating {target}…"),
                SubmoduleAction::Sync => format!("Syncing {target}…"),
            });
            state.error = None;
            cx.notify();
            state.recursive
        });
        let askpass = Self::askpass_env(cx);
        spawn_bg(
            cx,
            move || match action {
                SubmoduleAction::Init => corvene_git::submodule_init(git, &workdir, &paths),
                SubmoduleAction::Update => corvene_git::submodule_update(
                    git,
                    &workdir,
                    &paths,
                    recursive,
                    askpass.as_ref(),
                ),
                SubmoduleAction::Sync => {
                    corvene_git::submodule_sync(git, &workdir, &paths, recursive)
                }
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    if let Some(state) = s.repo_state_mut(id).submodules.as_mut() {
                        state.busy = None;
                        state.error = result.err().map(|err| match &err {
                            corvene_git::GitError::Failed { stderr, .. } => {
                                stderr.trim().to_string()
                            }
                            _ => err.to_string(),
                        });
                        cx.notify();
                    }
                });
                Self::load_submodules(id, cx);
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// Open as Repository: the submodule is added (or selected) as a
    /// repository of its own and the panel closes.
    pub fn open_submodule_repository(id: u64, path: String, cx: &mut dyn Host) {
        let Some(workdir) = Self::repo_context(id, cx).map(|(_, workdir)| workdir) else {
            return;
        };
        Self::close_submodules(id, cx);
        Self::open_submodule(PathBuf::from(&workdir).join(path), cx);
    }

    pub fn close_submodules(id: u64, cx: &mut dyn Host) {
        Self::close_popup_if(|p| matches!(p, Popup::Submodules { .. }), cx);
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).submodules = None);
    }
}
