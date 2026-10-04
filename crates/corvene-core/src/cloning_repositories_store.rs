//! GHD `CloningRepositoriesStore`
//! (`app/src/lib/stores/cloning-repositories-store.ts`): the clones in
//! progress, each a [`CloneState`] (GHD's `CloningRepository` together with
//! its `ICloneProgress`), told apart by [`CloneState::id`].
//!
//! `Dispatcher::clone_repository_with` runs the clone (GHD's `clone`) and
//! keeps this store up to date: [`CloningRepositoriesStore::push`] when it
//! starts, [`CloningRepositoriesStore::update_progress`] for each progress
//! line and [`CloningRepositoriesStore::remove`] when it ends.
//!
//! Differences from GHD:
//!
//! - Clones are not rows of the repository list, so none is ever selected:
//!   the content area shows the newest one ([`CloningRepositoriesStore::latest`])
//!   where GHD shows the selected cloning repository (the one a new clone
//!   selects).
//! - GHD keeps the progress apart from the repository (`stateByID`, empty
//!   for a repository put in the list without starting a clone); a
//!   `CloneState` carries its own progress, so
//!   [`CloningRepositoriesStore::get_repository_state`] answers for every
//!   clone in the list.
//! - The app re-renders through the `AppState` entity's `cx.notify()`;
//!   listeners added with [`CloningRepositoriesStore::on_did_update`] (GHD
//!   `BaseStore.onDidUpdate`) are told about the same changes for code that
//!   has no gpui context.

use crate::state::CloneState;

/// GHD `CloningRepositoriesStore`.
#[derive(Default)]
pub struct CloningRepositoriesStore {
    repositories: Vec<CloneState>,
    listeners: Vec<Box<dyn FnMut()>>,
}

impl CloningRepositoriesStore {
    /// `new CloningRepositoriesStore()`: no clones.
    pub fn new() -> Self {
        Self::default()
    }

    /// GHD `repositories`: the clones in progress, oldest first.
    pub fn repositories(&self) -> &[CloneState] {
        &self.repositories
    }

    /// The clone the content area shows: the one started last (GHD shows
    /// the selected cloning repository, which a new clone selects).
    pub fn latest(&self) -> Option<&CloneState> {
        self.repositories.last()
    }

    /// The clone with this [`CloneState::id`].
    pub fn get(&self, id: u64) -> Option<&CloneState> {
        self.repositories.iter().find(|r| r.id == id)
    }

    /// GHD `clone`'s start: put a clone in the list and tell the listeners.
    pub fn push(&mut self, repository: CloneState) {
        self.repositories.push(repository);
        self.emit_update();
    }

    /// GHD `getRepositoryState`: the clone's progress (its
    /// [`CloneState::description`] and [`CloneState::value`]), `None` for a
    /// clone that is not in the list.
    pub fn get_repository_state(&self, repository: &CloneState) -> Option<&CloneState> {
        self.get(repository.id)
    }

    /// GHD `clone`'s progress callback: record a progress line for the clone
    /// with this id. Returns `false` when the clone is gone or cancelled
    /// (`234-clone-cancel` shows "Cancelling…" until git exits), so the
    /// progress pump can stop.
    pub fn update_progress(&mut self, id: u64, description: String, value: Option<f32>) -> bool {
        let Some(clone) = self
            .repositories
            .iter_mut()
            .find(|r| r.id == id && !r.cancel.is_cancelled())
        else {
            return false;
        };
        clone.description = description;
        clone.value = value;
        self.emit_update();
        true
    }

    /// Corvene `234-clone-cancel`: stop the clone with this id; it says
    /// "Cancelling…" until git exits and the dispatcher removes it.
    pub fn cancel(&mut self, id: u64) -> bool {
        let Some(clone) = self.repositories.iter_mut().find(|r| r.id == id) else {
            return false;
        };
        clone.cancel.cancel();
        clone.description = "Cancelling…".into();
        clone.value = None;
        self.emit_update();
        true
    }

    /// GHD `remove`: drop the clone (if it is in the list) and tell the
    /// listeners either way.
    pub fn remove(&mut self, repository: &CloneState) {
        self.remove_id(repository.id);
    }

    /// [`Self::remove`] by [`CloneState::id`].
    pub fn remove_id(&mut self, id: u64) {
        self.repositories.retain(|r| r.id != id);
        self.emit_update();
    }

    /// GHD `onDidUpdate`: call `callback` after every change.
    pub fn on_did_update(&mut self, callback: impl FnMut() + 'static) {
        self.listeners.push(Box::new(callback));
    }

    /// GHD `emitUpdate`.
    fn emit_update(&mut self) {
        for listener in &mut self.listeners {
            listener();
        }
    }
}

impl std::fmt::Debug for CloningRepositoriesStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloningRepositoriesStore")
            .field("repositories", &self.repositories)
            .field("listeners", &self.listeners.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn clone(url: &str) -> CloneState {
        CloneState::new(PathBuf::from("/tmp/x"), url.to_string())
    }

    #[test]
    fn latest_is_the_newest_clone_and_progress_follows_the_id() {
        let mut store = CloningRepositoriesStore::new();
        let first = clone("https://github.com/owner/a.git");
        let second = clone("https://github.com/owner/b.git");
        store.push(first.clone());
        store.push(second.clone());
        assert_eq!(store.latest().map(|c| c.id), Some(second.id));

        assert!(store.update_progress(first.id, "Receiving objects".into(), Some(0.5)));
        assert_eq!(
            store.get(first.id).map(|c| c.description.as_str()),
            Some("Receiving objects")
        );
        assert_eq!(store.get(second.id).map(|c| c.value), Some(None));

        assert!(store.cancel(second.id));
        assert!(!store.update_progress(second.id, "late".into(), None));
        store.remove(&second);
        assert_eq!(store.latest().map(|c| c.id), Some(first.id));
    }
}
