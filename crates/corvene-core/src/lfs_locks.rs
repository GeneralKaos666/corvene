//! Corvene (`1113-lfs-locks`): Git LFS file locks. In a repository that
//! uses LFS (`RepositoryState::uses_lfs`, from `.gitattributes`), every
//! refresh asks the server for its locks at most once a minute
//! (`corvene_git::lfs_locks`); when the server has a locking API, the file
//! menus of Changes and History get Lock File / Unlock File, locked files
//! show a lock with the holder's name, and a repository admin (GitHub's
//! `permissions.admin`) is offered Force Unlock for someone else's lock,
//! confirmed first (`Popup::ConfirmForceUnlock`). GHD has none of this.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use corvene_git::{LfsLock, LfsLocks};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup, RepositoryState};

/// How long a lock list counts as fresh for refreshes.
const LOCKS_TTL: Duration = Duration::from_secs(60);

/// The server's answer (`RepositoryState::lfs_locks`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LfsLockState {
    /// The server has a locking API.
    pub supported: bool,
    /// Path → lock.
    pub locks: Arc<HashMap<String, LfsLock>>,
    pub loaded_at: Instant,
    pub loading: bool,
    /// A lock or unlock ran: the next refresh asks again.
    pub stale: bool,
}

impl RepositoryState {
    /// `1113-lfs-locks`: locks can be taken here (LFS, and a server with
    /// locking). The flag is checked by the caller.
    pub fn lfs_locking(&self) -> bool {
        self.uses_lfs && self.lfs_locks.as_ref().is_some_and(|l| l.supported)
    }

    /// `1113-lfs-locks`: the lock on `path`, if any.
    pub fn lfs_lock(&self, path: &str) -> Option<&LfsLock> {
        self.lfs_locks
            .as_ref()
            .filter(|_| self.uses_lfs)
            .and_then(|l| l.locks.get(path))
    }
}

impl AppState {
    /// `1113-lfs-locks`: the user administers repository `id` on GitHub,
    /// so may release other people's locks.
    pub fn is_repository_admin(&self, id: u64) -> bool {
        self.repository(id)
            .and_then(|r| r.github.as_ref())
            .and_then(|gh| gh.permissions)
            == Some(corvene_models::RepositoryPermission::Admin)
    }
}

impl Dispatcher {
    /// Ask the LFS server for its locks: after a refresh (`force` off: at
    /// most once per [`LOCKS_TTL`]) and after a lock or unlock (`force`).
    pub fn refresh_lfs_locks(id: u64, force: bool, cx: &mut dyn Host) {
        let go = {
            let s = Self::state(cx).read(cx);
            s.flags.bool(crate::flags::ids::LFS_LOCKS)
                && s.repo_states.get(&id).is_some_and(|rs| {
                    rs.uses_lfs
                        && match &rs.lfs_locks {
                            None => true,
                            Some(l) => {
                                !l.loading
                                    && (force || l.stale || l.loaded_at.elapsed() >= LOCKS_TTL)
                            }
                        }
                })
        };
        if !go {
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            match rs.lfs_locks.as_mut() {
                Some(l) => l.loading = true,
                None => {
                    rs.lfs_locks = Some(LfsLockState {
                        supported: false,
                        locks: Arc::default(),
                        loaded_at: Instant::now(),
                        loading: true,
                        stale: false,
                    })
                }
            }
        });
        let askpass = Self::askpass_env_for_repository(id, cx);
        spawn_bg(
            cx,
            move || corvene_git::lfs_locks(git, &workdir, askpass.as_ref()),
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(state) = s.repo_state_mut(id).lfs_locks.as_mut() else {
                        return;
                    };
                    state.loading = false;
                    state.loaded_at = Instant::now();
                    state.stale = false;
                    let (supported, locks) = match result {
                        Ok(LfsLocks::Locks(locks)) => (true, locks),
                        Ok(LfsLocks::Unsupported) => (false, Vec::new()),
                        // offline or signed out: keep what was known
                        Err(err) => {
                            tracing::debug!(id, %err, "could not read LFS locks");
                            return;
                        }
                    };
                    let locks: HashMap<String, LfsLock> =
                        locks.into_iter().map(|l| (l.path.clone(), l)).collect();
                    if state.supported != supported || *state.locks != locks {
                        state.supported = supported;
                        state.locks = Arc::new(locks);
                        cx.notify();
                    }
                });
            },
        );
    }

    /// Lock File(s): `git lfs lock` each path.
    pub fn lock_lfs_files(id: u64, paths: Vec<String>, cx: &mut dyn Host) {
        let askpass = Self::askpass_env_for_repository(id, cx);
        Self::mark_lfs_locks_stale(id, cx);
        // the refresh after it asks for the locks again
        Self::run_history_op(
            id,
            "Could not lock the file",
            move |git, workdir| corvene_git::lfs_lock(git, &workdir, &paths, askpass.as_ref()),
            cx,
        );
    }

    /// Unlock File(s) (`force`: Force Unlock, someone else's lock).
    pub fn unlock_lfs_files(id: u64, paths: Vec<String>, force: bool, cx: &mut dyn Host) {
        let ids: Vec<String> = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            paths
                .iter()
                .filter_map(|p| rs.lfs_lock(p).map(|l| l.id.clone()))
                .collect()
        };
        if ids.is_empty() {
            return;
        }
        let askpass = Self::askpass_env_for_repository(id, cx);
        Self::mark_lfs_locks_stale(id, cx);
        Self::run_history_op(
            id,
            "Could not unlock the file",
            move |git, workdir| {
                corvene_git::lfs_unlock(git, &workdir, &ids, force, askpass.as_ref())
            },
            cx,
        );
    }

    fn mark_lfs_locks_stale(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, _| {
            if let Some(l) = s.repo_state_mut(id).lfs_locks.as_mut() {
                l.stale = true;
            }
        });
    }

    /// Force Unlock…: confirm releasing `path`'s lock, held by someone else.
    pub fn request_force_unlock(id: u64, path: String, cx: &mut dyn Host) {
        let owner = {
            let s = Self::state(cx).read(cx);
            if !s.is_repository_admin(id) {
                return;
            }
            match s.repo_states.get(&id).and_then(|rs| rs.lfs_lock(&path)) {
                Some(lock) => lock.owner.clone(),
                None => return,
            }
        };
        Self::show_popup(
            Popup::ConfirmForceUnlock {
                repo: id,
                path,
                owner,
            },
            cx,
        );
    }
}
