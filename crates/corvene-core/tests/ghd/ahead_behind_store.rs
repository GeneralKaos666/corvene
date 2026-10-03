//! Port of GitHub Desktop's `app/test/unit/ahead-behind-store-test.ts`.
//!
//! GitHub Desktop's `AheadBehindStore` (`lib/stores/ahead-behind-store.ts`)
//! counts `from...to` in a background worker (`getAheadBehind(repository,
//! revSymmetricDifference(from, to))`), caches the result per repository
//! and range, and calls back unless the request was disposed first. It feeds
//! the compare branch list. Corvene's counterpart:
//!
//! - the count is `corvene_git::symmetric_ahead_behind(git, path, from,
//!   to)` ([`wait_for_ahead_behind`]);
//! - the cache is the repository's `CompareState::branch_counts`, keyed by
//!   the other branch's name (`to`) relative to the current branch
//!   (`from`), so a new store is `CompareState::default()` and
//!   `tryGetAheadBehind` is a lookup in it ([`try_get_ahead_behind`]);
//! - only `Dispatcher::load_compare_counts` (needs a gpui `App`) fills that
//!   cache, one `symmetric_ahead_behind` per branch, with no way to cancel a
//!   request: [`get_ahead_behind`] is a stand-in for the store's
//!   cancellable, caching request.

use std::path::Path;

use corvene_core::AheadBehind;
use corvene_core::compare::CompareState;
use corvene_test_support::{
    Tree, TreeEntry, create_branch, exec, git, make_commit, setup_empty_repository, switch_to,
};

/// GitHub Desktop's `AheadBehindStore`: Corvene's cache of counts.
type AheadBehindStore = CompareState;

/// The `IDisposable` GitHub Desktop's `getAheadBehind` returns.
struct Disposable;

impl Disposable {
    fn dispose(self) {
        unimplemented!("Corvene's ahead/behind requests cannot be cancelled")
    }
}

/// Stand-in for GitHub Desktop's `AheadBehindStore.getAheadBehind(repository,
/// from, to, callback)`: count in the background, cache the result and call
/// `callback` unless the returned handle was disposed first. Corvene fills
/// `CompareState::branch_counts` only from `Dispatcher::load_compare_counts`
/// (needs a gpui `App`) and cannot cancel it; replace this with a gpui-free
/// call once there is one and remove the `#[ignore]`s.
fn get_ahead_behind(
    _store: &mut AheadBehindStore,
    _repo: &Path,
    _from: &str,
    _to: &str,
    _callback: impl FnOnce(AheadBehind) + 'static,
) -> Disposable {
    unimplemented!(
        "no gpui-free, cancellable ahead/behind request (Dispatcher::load_compare_counts)"
    )
}

/// GitHub Desktop's `tryGetAheadBehind(repository, from, to)`: the cached
/// count, if any. Corvene caches per repository by the other branch, `to`.
fn try_get_ahead_behind(
    store: &AheadBehindStore,
    _repo: &Path,
    _from: &str,
    to: &str,
) -> Option<AheadBehind> {
    store.branch_counts.get(to).copied()
}

/// The test's `getSHA(repo)`: `git rev-parse HEAD`.
fn get_sha(repo: &Path) -> String {
    exec(["rev-parse", "HEAD"], repo).stdout.trim().to_string()
}

/// The test's `waitForAheadBehind(store, repo, from, to)`: the count the
/// store's worker makes for `from...to`, `symmetric_ahead_behind`. It does
/// not go through Corvene's cache, which nothing outside the dispatcher
/// fills (see the module docs).
fn wait_for_ahead_behind(
    _store: &mut AheadBehindStore,
    repo: &Path,
    from: &str,
    to: &str,
) -> Option<AheadBehind> {
    corvene_git::symmetric_ahead_behind(git(), repo, from, to)
        .unwrap_or_else(|err| panic!("ahead/behind for range {from}...{to}: {err}"))
}

/// `beforeEach`: `store = new AheadBehindStore()`.
fn store() -> AheadBehindStore {
    CompareState::default()
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
#[ignore = "ghd: missing: no gpui-free ahead/behind cache; counts reach CompareState::branch_counts only through Dispatcher::load_compare_counts (needs a gpui App), symmetric_ahead_behind caches nothing, so tryGetAheadBehind finds None"]
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
#[ignore = "ghd: missing: no cancellable ahead/behind request; Dispatcher::load_compare_counts (needs a gpui App) returns no handle to dispose (GHD AheadBehindStore.getAheadBehind returns an IDisposable)"]
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
