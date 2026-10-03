//! Helpers shared by the `sync` lane's `corvene-git` modules (GitHub
//! Desktop's `app/test/unit/git/{fetch,push,remote}-test.ts` and
//! `unit/git/pull/*-test.ts`).

use corvene_git::GitError;
use corvene_models::Remote;
use corvene_test_support::{TestRepo, git};

/// `featureBranch` of GitHub Desktop's `unit/git/pull/*-test.ts`.
pub const FEATURE_BRANCH: &str = "this-is-a-feature";

/// `const remote: IRemote = { name: 'origin', url: 'file://' }` of GitHub
/// Desktop's `unit/git/pull/*-test.ts` (the URL only feeds
/// `envForRemoteOperation`; git uses the name).
pub fn remote() -> Remote {
    Remote {
        name: "origin".into(),
        url: "file://".into(),
    }
}

/// `remoteBranch = `${remote.name}/${featureBranch}`` of GitHub Desktop's
/// `unit/git/pull/*-test.ts`.
pub fn remote_branch() -> String {
    format!("{}/{}", remote().name, FEATURE_BRANCH)
}

/// GitHub Desktop's `fetch(repository, remote)` (`lib/git/fetch.ts`, no
/// progress callback, user initiated): `corvene_git::fetch` without askpass
/// and with a progress callback that drops every event (Corvene always
/// passes `--progress`, which only changes what git writes to stderr).
///
/// # Panics
///
/// When git fails (GitHub Desktop's `fetch` rejects, which fails the test).
pub fn fetch(repository: &TestRepo, remote: &Remote) {
    corvene_git::fetch(git(), repository.path(), &remote.name, None, &mut |_, _| {})
        .unwrap_or_else(|err| panic!("fetch {}: {err}", remote.name));
}

/// GitHub Desktop's `pull(repository, remote)` (`lib/git/pull.ts`, no
/// options): `corvene_git::pull` with submodules followed (GitHub Desktop's
/// `--recurse-submodules`; `250-sync-skips-submodules` off, its GitHub
/// Desktop value), no askpass and a progress callback that drops every
/// event.
pub fn pull(repository: &TestRepo, remote: &Remote) -> Result<(), GitError> {
    corvene_git::pull(
        git(),
        repository.path(),
        &remote.name,
        false,
        None,
        &mut |_, _| {},
    )
}
