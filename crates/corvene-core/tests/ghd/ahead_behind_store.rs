//! Port of GitHub Desktop's `app/test/unit/ahead-behind-store-test.ts`.
//!
//! GitHub Desktop's `AheadBehindStore` (`lib/stores/ahead-behind-store.ts`)
//! counts `from...to` in a background worker (`getAheadBehind(repository,
//! revSymmetricDifference(from, to))`), caches the result per repository
//! and range, and calls back unless the request was disposed first. It feeds
//! the compare branch list. It is
//! `corvene_core::ahead_behind_store::AheadBehindStore`
//! (`AppState::ahead_behind`, which `Dispatcher::load_compare_counts` asks
//! for every branch): its callbacks run on the thread that owns the store,
//! at once for a cached range and otherwise from `poll` / `wait` once the
//! background worker has counted the range, so the test's
//! `waitForAheadBehind` subscribes and then `wait`s.

use std::path::Path;

use corvene_core::AheadBehind;
use corvene_core::ahead_behind_store::{AheadBehindStore, Disposable};
use corvene_test_support::{
    Tree, TreeEntry, create_branch, exec, git, make_commit, setup_empty_repository, switch_to,
};

/// GitHub Desktop's `AheadBehindStore.getAheadBehind(repository, from, to,
/// callback)`.
fn get_ahead_behind(
    store: &mut AheadBehindStore,
    repo: &Path,
    from: &str,
    to: &str,
    callback: impl FnOnce(AheadBehind) + 'static,
) -> Disposable {
    store.get_ahead_behind(git(), repo, from, to, callback)
}

/// GitHub Desktop's `tryGetAheadBehind(repository, from, to)`.
fn try_get_ahead_behind(
    store: &AheadBehindStore,
    repo: &Path,
    from: &str,
    to: &str,
) -> Option<AheadBehind> {
    store.try_get_ahead_behind(repo, from, to)
}

/// The test's `getSHA(repo)`: `git rev-parse HEAD`.
fn get_sha(repo: &Path) -> String {
    exec(["rev-parse", "HEAD"], repo).stdout.trim().to_string()
}

/// The test's `waitForAheadBehind(store, repo, from, to)`: subscribe to
/// `from...to` and wait for the callback (`wait` blocks until the worker has
/// counted every requested range).
fn wait_for_ahead_behind(
    store: &mut AheadBehindStore,
    repo: &Path,
    from: &str,
    to: &str,
) -> Option<AheadBehind> {
    let result = std::rc::Rc::new(std::cell::Cell::new(None));
    let slot = result.clone();
    get_ahead_behind(store, repo, from, to, move |ahead_behind| {
        slot.set(Some(ahead_behind))
    });
    store.wait();
    result.get()
}

/// `beforeEach`: `store = new AheadBehindStore()`.
fn store() -> AheadBehindStore {
    AheadBehindStore::new()
}

// GHD: unit/ahead-behind-store-test.ts › AheadBehindStore › tryGetAheadBehind › returns undefined for uncached range
#[test]
fn returns_undefined_for_uncached_range() {
    let store = store();
    let repo = Path::new("/fake/path");
    let result = try_get_ahead_behind(&store, repo, "abc123", "def456");
    assert_eq!(result, None);
}

// GHD: unit/ahead-behind-store-test.ts › AheadBehindStore › getAheadBehind › calculates ahead/behind for diverged branches
#[test]
fn calculates_ahead_behind_for_diverged_branches() {
    let mut store = store();
    let repo = setup_empty_repository();

    // Create initial commit on master
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("base.txt", "base")]),
    );

    // Create a feature branch and add a commit
    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message(
            "feature commit",
            [TreeEntry::new("feature.txt", "feature work")],
        ),
    );
    let feature_sha = get_sha(repo.path());

    // Go back to master and add a different commit
    switch_to(&repo, "master");
    make_commit(
        &repo,
        &Tree::with_message("main commit", [TreeEntry::new("main.txt", "main work")]),
    );
    let main_sha = get_sha(repo.path());

    // Now get the ahead/behind count
    let result = wait_for_ahead_behind(&mut store, repo.path(), &main_sha, &feature_sha);

    assert_ne!(result, None);
    let result = result.unwrap();
    // master is 1 ahead (its own commit) and 1 behind (the feature commit)
    assert_eq!(result.ahead, 1);
    assert_eq!(result.behind, 1);
}

// GHD: unit/ahead-behind-store-test.ts › AheadBehindStore › getAheadBehind › returns cached result on subsequent calls
#[test]
fn returns_cached_result_on_subsequent_calls() {
    let mut store = store();
    let repo = setup_empty_repository();

    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("base.txt", "base")]),
    );

    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message("feature commit", [TreeEntry::new("feature.txt", "feature")]),
    );
    let feature_sha = get_sha(repo.path());

    switch_to(&repo, "master");
    let main_sha = get_sha(repo.path());

    // First call — populates cache
    wait_for_ahead_behind(&mut store, repo.path(), &main_sha, &feature_sha);

    // Second call — should return cached result synchronously
    let cached = try_get_ahead_behind(&store, repo.path(), &main_sha, &feature_sha);
    assert_ne!(cached, None);
    let cached = cached.unwrap();
    // master is 0 ahead, 1 behind (feature has 1 extra commit)
    assert_eq!(cached.ahead, 0);
    assert_eq!(cached.behind, 1);
}

// GHD: unit/ahead-behind-store-test.ts › AheadBehindStore › getAheadBehind › supports aborting via disposable
#[test]
fn supports_aborting_via_disposable() {
    let mut store = store();
    let repo = setup_empty_repository();

    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("base.txt", "base")]),
    );

    create_branch(&repo, "feature", "HEAD");
    switch_to(&repo, "feature");
    make_commit(
        &repo,
        &Tree::with_message("feature commit", [TreeEntry::new("feature.txt", "feature")]),
    );
    let feature_sha = get_sha(repo.path());

    switch_to(&repo, "master");
    let main_sha = get_sha(repo.path());

    let callback_called = std::rc::Rc::new(std::cell::Cell::new(false));
    let called = callback_called.clone();
    let disposable = get_ahead_behind(
        &mut store,
        repo.path(),
        &main_sha,
        &feature_sha,
        move |_| {
            called.set(true);
        },
    );

    // Immediately dispose — should prevent callback
    disposable.dispose();

    // Confirm the underlying worker completed and cached its result.
    wait_for_ahead_behind(&mut store, repo.path(), &main_sha, &feature_sha);

    assert!(!callback_called.get());
}
