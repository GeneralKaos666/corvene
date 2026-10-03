//! Port of GitHub Desktop's `app/test/unit/cloning-repositories-store-test.ts`.
//!
//! GitHub Desktop's `CloningRepositoriesStore`
//! (`lib/stores/cloning-repositories-store.ts`) lists the clones in progress
//! (`repositories`), their progress (`getRepositoryState`), removes them
//! (`remove`) and tells listeners (`onDidUpdate`). Corvene keeps at most one
//! clone, `AppState::cloning: Option<CloneState>`, which only
//! `Dispatcher::clone_repository_with` sets and clears (needs a gpui `App`);
//! its progress is the `CloneState`'s `description` / `value`, and a change
//! is announced by the state entity's `cx.notify()`. There is no gpui-free
//! store, so [`CloningRepositoriesStore`] is a stand-in with GitHub
//! Desktop's methods; `new CloningRepository(path, url)` is a
//! [`CloneState`] (see `cloning_repository.rs`).

use corvene_core::CloneState;

use crate::cloning_support::cloning_repository;

/// Stand-in for GitHub Desktop's `CloningRepositoriesStore`. Replace its
/// methods with gpui-free Corvene calls once there are some and remove the
/// `#[ignore]`s.
struct CloningRepositoriesStore;

impl CloningRepositoriesStore {
    /// `new CloningRepositoriesStore()`.
    fn new() -> Self {
        unimplemented!(
            "no gpui-free clone store (AppState::cloning, Dispatcher::clone_repository_with)"
        )
    }

    /// `store.repositories`.
    fn repositories(&self) -> Vec<CloneState> {
        unimplemented!("no gpui-free clone store")
    }

    /// `(store as any)._repositories.push(repo)`: a clone that has started.
    fn push(&mut self, _repository: CloneState) {
        unimplemented!("no gpui-free clone store")
    }

    /// `store.remove(repository)`.
    fn remove(&mut self, _repository: &CloneState) {
        unimplemented!("no gpui-free clone store")
    }

    /// `store.getRepositoryState(repository)`: the clone's progress
    /// (Corvene: `description` and `value`).
    fn get_repository_state(&self, _repository: &CloneState) -> Option<(String, Option<f32>)> {
        unimplemented!("no gpui-free clone store")
    }

    /// `store.onDidUpdate(callback)`.
    fn on_did_update(&mut self, _callback: impl FnMut() + 'static) {
        unimplemented!("no gpui-free clone store")
    }
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › starts with no repositories
#[test]
#[ignore = "ghd: missing: no gpui-free clone store; the clone in progress is AppState::cloning, set and cleared only by Dispatcher::clone_repository_with (needs a gpui App)"]
fn starts_with_no_repositories() {
    let store = CloningRepositoriesStore::new();
    assert_eq!(store.repositories().len(), 0);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › remove › removes a repository that was added
#[test]
#[ignore = "ghd: missing: no gpui-free clone store; AppState::cloning is cleared only when Dispatcher::clone_repository_with's clone ends (needs a gpui App)"]
fn removes_a_repository_that_was_added() {
    let mut store = CloningRepositoriesStore::new();
    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");

    // Manually push to simulate the clone starting
    store.push(repo.clone());
    assert_eq!(store.repositories().len(), 1);

    store.remove(&repo);
    assert_eq!(store.repositories().len(), 0);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › remove › handles removing a repository that does not exist
#[test]
#[ignore = "ghd: missing: no gpui-free clone store; AppState::cloning is cleared only when Dispatcher::clone_repository_with's clone ends (needs a gpui App)"]
fn handles_removing_a_repository_that_does_not_exist() {
    let mut store = CloningRepositoriesStore::new();
    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");

    // Should not throw
    store.remove(&repo);
    assert_eq!(store.repositories().len(), 0);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › getRepositoryState › returns null for unknown repository
#[test]
#[ignore = "ghd: missing: no gpui-free clone store; a clone's progress is AppState::cloning's description/value, kept by Dispatcher::clone_repository_with (needs a gpui App)"]
fn returns_null_for_unknown_repository() {
    let store = CloningRepositoriesStore::new();
    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");
    assert_eq!(store.get_repository_state(&repo), None);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › emitUpdate › notifies listeners when state changes
#[test]
#[ignore = "ghd: missing: no gpui-free clone store; changes to AppState::cloning are announced by the gpui entity's cx.notify() in Dispatcher::clone_repository_with (needs a gpui App)"]
fn notifies_listeners_when_state_changes() {
    let mut store = CloningRepositoriesStore::new();
    let update_count = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = update_count.clone();
    store.on_did_update(move || {
        count.set(count.get() + 1);
    });

    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");
    store.remove(&repo); // triggers emitUpdate

    assert!(
        update_count.get() > 0,
        "Expected at least one update notification"
    );
}
