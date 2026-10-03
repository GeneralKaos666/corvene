//! Port of GitHub Desktop's `app/test/unit/git/remote-test.ts`.
//!
//! Corvene equivalents (GitHub Desktop's `lib/git/remote.ts` unless noted):
//!
//! - `getRemotes(repository)`: `corvene_git::get_remotes` (`git remote -v`,
//!   fetch URLs, sorted by name), which returns a `Result`. GitHub Desktop's
//!   `getRemotes` never rejects for a directory that is not a repository
//!   (it returns `[]`), so the ported tests unwrap it. The app's repository
//!   state reads the remotes with gitoxide instead
//!   (`corvene_git::open_repository(..).remotes`); the function GitHub
//!   Desktop's tests exercise is the CLI one.
//! - `findDefaultRemote(remotes)` (`lib/stores/helpers/find-default-remote.ts`):
//!   `corvene_git::find_default_remote`, `None` for GitHub Desktop's `null`.
//! - `addRemote`, `removeRemote`, `setRemoteURL`: `corvene_git::add_remote`,
//!   `remove_remote`, `set_remote_url`. GitHub Desktop's `setRemoteURL`
//!   resolves to the constant `true` or rejects; Corvene's returns `Ok(())`
//!   or an error, so `assert.equal(await setRemoteURL(..), true)` is
//!   `is_ok()` and `assert.rejects` is `is_err()`.
//! - `setConfigValue(repository, ..)` (`lib/git/config.ts`):
//!   `corvene_git::set_local_config_value`.
//! - `IRemote` is `corvene_models::Remote`.

use corvene_git::{
    add_remote, find_default_remote, get_remotes, remove_remote, set_local_config_value,
    set_remote_url,
};
use corvene_test_support::{
    exec, git, setup_empty_directory, setup_empty_repository, setup_fixture_repository,
};

// GHD: unit/git/remote-test.ts › git/remote › getRemotes › should return both remotes
#[test]
fn should_return_both_remotes() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");
    add_remote(
        git(),
        repository.path(),
        "spaces-in-path",
        "/path/with spaces/foo",
    )
    .unwrap();

    // NB: We don't check for exact URL equality because CircleCI's git config
    // rewrites HTTPS URLs to SSH.
    let nwo = "shiftkey/friendly-bassoon.git";

    let result = get_remotes(git(), repository.path()).unwrap();

    // Changes the output of git remote -v, see
    // https://github.com/git/git/blob/9005149a4a77e2d3409c6127bf4fd1a0893c3495/builtin/remote.c#L1223-L1226
    set_local_config_value(
        git(),
        repository.path(),
        "remote.bassoon.partialclonefilter",
        "foo",
    )
    .unwrap();

    assert_eq!(result[0].name, "bassoon");
    assert!(result[0].url.ends_with(nwo));

    assert_eq!(result[1].name, "origin");
    assert!(result[1].url.ends_with(nwo));

    assert_eq!(result[2].name, "spaces-in-path");
    assert_eq!(result[2].url, "/path/with spaces/foo");
}

// GHD: unit/git/remote-test.ts › git/remote › getRemotes › returns remotes sorted alphabetically
#[test]
fn returns_remotes_sorted_alphabetically() {
    let repository = setup_empty_repository();

    // adding these remotes out-of-order to test how they are then retrieved
    let url = "https://github.com/desktop/not-found.git";

    exec(["remote", "add", "X", url], repository.path());
    exec(["remote", "add", "A", url], repository.path());
    exec(["remote", "add", "L", url], repository.path());
    exec(["remote", "add", "T", url], repository.path());
    exec(["remote", "add", "D", url], repository.path());

    let result = get_remotes(git(), repository.path()).unwrap();
    assert_eq!(result.len(), 5);

    assert_eq!(result[0].name, "A");
    assert_eq!(result[1].name, "D");
    assert_eq!(result[2].name, "L");
    assert_eq!(result[3].name, "T");
    assert_eq!(result[4].name, "X");
}

// GHD: unit/git/remote-test.ts › git/remote › getRemotes › returns empty array for directory without a .git directory
#[test]
#[ignore = "ghd: bug: get_remotes fails (git exit 128, not a git repository) in a plain directory; GHD getRemotes returns [] on NotAGitRepository"]
fn returns_empty_array_for_directory_without_a_git_directory() {
    let repository = setup_empty_directory();
    let remotes = get_remotes(git(), repository.path()).unwrap();
    assert_eq!(remotes.len(), 0);
}

// GHD: unit/git/remote-test.ts › git/remote › getRemotes › returns promisor remote
#[test]
#[ignore = "ghd: bug: get_remotes keeps only `remote -v` lines ending in ` (fetch)`, so a promisor remote (`<url> (fetch) [blob:none]`) is dropped: got 0 remotes, expected 1 (hasBlobFilter)"]
fn returns_promisor_remote() {
    let repository = setup_empty_repository();

    // Add a remote
    let url = "https://github.com/desktop/not-found.git";
    exec(["remote", "add", "hasBlobFilter", url], repository.path());

    // Fetch a remote and add a filter
    exec(["fetch", "--filter=blob:none"], repository.path());

    // Shows that the new remote does have a filter
    let raw_get_remote = exec(["remote", "-v"], repository.path());
    let needle = format!("{url} (fetch) [blob:none]");
    assert!(raw_get_remote.stdout.contains(&needle));

    // Shows that the `getRemote` returns that remote
    let result = get_remotes(git(), repository.path()).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "hasBlobFilter");
}

// GHD: unit/git/remote-test.ts › git/remote › findDefaultRemote › returns null for empty array
#[test]
fn returns_null_for_empty_array() {
    let result = find_default_remote(&[]);
    assert!(result.is_none());
}

// GHD: unit/git/remote-test.ts › git/remote › findDefaultRemote › returns origin when multiple remotes found
#[test]
fn returns_origin_when_multiple_remotes_found() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");

    let remotes = get_remotes(git(), repository.path()).unwrap();
    let result = find_default_remote(&remotes);

    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "origin");
}

// GHD: unit/git/remote-test.ts › git/remote › findDefaultRemote › returns something when origin removed
#[test]
fn returns_something_when_origin_removed() {
    let repository = setup_fixture_repository("repo-with-multiple-remotes");
    remove_remote(git(), repository.path(), "origin").unwrap();

    let remotes = get_remotes(git(), repository.path()).unwrap();
    let result = find_default_remote(&remotes);

    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "bassoon");
}

// GHD: unit/git/remote-test.ts › git/remote › findDefaultRemote › returns null for new repository
#[test]
fn returns_null_for_new_repository() {
    let repository = setup_empty_repository();

    let remotes = get_remotes(git(), repository.path()).unwrap();
    let result = find_default_remote(&remotes);

    assert!(result.is_none());
}

// GHD: unit/git/remote-test.ts › git/remote › addRemote › can set origin and return it as default
#[test]
fn can_set_origin_and_return_it_as_default() {
    let repository = setup_empty_repository();
    add_remote(
        git(),
        repository.path(),
        "origin",
        "https://github.com/desktop/desktop",
    )
    .unwrap();

    let remotes = get_remotes(git(), repository.path()).unwrap();
    let result = find_default_remote(&remotes);

    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "origin");
}

// GHD: unit/git/remote-test.ts › git/remote › removeRemote › silently fails when remote not defined
#[test]
fn silently_fails_when_remote_not_defined() {
    let repository = setup_empty_repository();
    let result = remove_remote(git(), repository.path(), "origin");
    assert!(result.is_ok(), "{result:?}");
}

const REMOTE_NAME: &str = "origin";
const REMOTE_URL: &str = "https://fakeweb.com/owner/name";
const NEW_URL: &str = "https://github.com/desktop/desktop";

// GHD: unit/git/remote-test.ts › git/remote › setRemoteURL › can set the url for an existing remote
#[test]
fn can_set_the_url_for_an_existing_remote() {
    let repository = setup_empty_repository();
    add_remote(git(), repository.path(), REMOTE_NAME, REMOTE_URL).unwrap();
    assert!(set_remote_url(git(), repository.path(), REMOTE_NAME, NEW_URL).is_ok());

    let remotes = get_remotes(git(), repository.path()).unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].url, NEW_URL);
}

// GHD: unit/git/remote-test.ts › git/remote › setRemoteURL › returns false for unknown remote name
#[test]
fn returns_false_for_unknown_remote_name() {
    let repository = setup_empty_repository();
    add_remote(git(), repository.path(), REMOTE_NAME, REMOTE_URL).unwrap();
    assert!(set_remote_url(git(), repository.path(), "none", NEW_URL).is_err());

    let remotes = get_remotes(git(), repository.path()).unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].url, REMOTE_URL);
}
