//! Port of GitHub Desktop's `app/test/unit/cloning-repositories-store-test.ts`.
//!
//! GitHub Desktop's `CloningRepositoriesStore`
//! (`lib/stores/cloning-repositories-store.ts`) lists the clones in progress
//! (`repositories`), their progress (`getRepositoryState`), removes them
//! (`remove`) and tells listeners (`onDidUpdate`). It is
//! `corvene_core::cloning_repositories_store::CloningRepositoriesStore`
//! (`AppState::cloning`, kept by `Dispatcher::clone_repository_with`); a
//! clone's progress is its `CloneState`'s `description` / `value`, and
//! `new CloningRepository(path, url)` is `CloneState::new` (see
//! `cloning_repository.rs`). `(store as any)._repositories.push(repo)` is
//! `push`, which also tells the listeners.

use corvene_core::cloning_repositories_store::CloningRepositoriesStore;

use crate::cloning_support::cloning_repository;

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › starts with no repositories
#[test]
fn starts_with_no_repositories() {
    let store = CloningRepositoriesStore::new();
    assert_eq!(store.repositories().len(), 0);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › remove › removes a repository that was added
#[test]
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
fn handles_removing_a_repository_that_does_not_exist() {
    let mut store = CloningRepositoriesStore::new();
    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");

    // Should not throw
    store.remove(&repo);
    assert_eq!(store.repositories().len(), 0);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › getRepositoryState › returns null for unknown repository
#[test]
fn returns_null_for_unknown_repository() {
    let store = CloningRepositoriesStore::new();
    let repo = cloning_repository("/tmp/test", "https://github.com/owner/repo.git");
    assert_eq!(store.get_repository_state(&repo), None);
}

// GHD: unit/cloning-repositories-store-test.ts › CloningRepositoriesStore › emitUpdate › notifies listeners when state changes
#[test]
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
