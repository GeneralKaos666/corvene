//! Port of GitHub Desktop's `app/test/unit/git-store-cache-test.ts`.
//!
//! GitHub Desktop's `GitStoreCache` (`lib/stores/git-store-cache.ts`) hands
//! out one `GitStore` per repository. Corvene has no `GitStore` objects:
//! what a `GitStore` holds lives in the repository's `RepositoryState`,
//! kept in `AppState::repo_states` and handed out (created on first use) by
//! `AppState::repo_state_mut`, which is `cache.get(repository)` here
//! ([`crate::stores_support::app_state`] builds the `AppState`).
//!
//! `cache.remove(repository)` is `AppState::remove_repo_state`, which
//! `Dispatcher::remove_repository` calls. GitHub Desktop compares object
//! identity; Rust may place the new state at the old one's address, so "a
//! different instance" is checked as "not the state that was removed": the
//! first state is tagged and the second must not carry the tag.

use std::path::PathBuf;

use corvene_core::Repository;
use corvene_core::state::RepositoryState;

use crate::stores_support::app_state;

/// `new Repository('/something/path', 1, null, false)`.
fn repository() -> Repository {
    Repository::new(1, PathBuf::from("/something/path"))
}

// GHD: unit/git-store-cache-test.ts › GitStoreCache › returns same instance of GitStore
#[test]
fn returns_same_instance_of_git_store() {
    let repository = repository();
    let mut cache = app_state();

    let first: *const RepositoryState = cache.repo_state_mut(repository.id);
    let second: *const RepositoryState = cache.repo_state_mut(repository.id);

    assert_eq!(first, second);
}

// GHD: unit/git-store-cache-test.ts › GitStoreCache › returns different instance of GitStore after removing
#[test]
fn returns_different_instance_of_git_store_after_removing() {
    let repository = repository();
    let mut cache = app_state();

    let first = cache.repo_state_mut(repository.id);
    // tag the first instance (see the module docs)
    first.selected_file = Some("first instance".to_string());
    cache.remove_repo_state(repository.id);
    let second = cache.repo_state_mut(repository.id);

    assert_ne!(second.selected_file.as_deref(), Some("first instance"));
}
