//! Port of GitHub Desktop's `app/test/unit/find-upstream-remote-test.ts`.
//!
//! GitHub Desktop's `findUpstreamRemote(parent, remotes)`
//! (`lib/stores/helpers/find-upstream-remote.ts`) is the remote named
//! `UpstreamRemoteName` (`upstream`) when it points at the fork's parent
//! (`repositoryMatchesRemote`), else `null`; the git store keeps it as
//! `upstreamRemote` and `addUpstreamRemoteIfNeeded` asks it first. Corvene
//! has `corvene_core::UPSTREAM_REMOTE_NAME` and
//! `corvene_models::url_matches_remote` (GHD `urlMatchesRemote` /
//! `repositoryMatchesRemote`) but no function that finds the upstream
//! remote: `Dispatcher::add_upstream_remote_if_needed` checks whether any
//! remote points at the parent instead. [`find_upstream_remote`] stands in
//! for it.
//!
//! [`git_hub_repo_fixture`] ports `helpers/github-repo-builder.ts`
//! `gitHubRepoFixture` for `corvene_models::GitHubRepository` (dotcom
//! endpoint, `htmlUrl` and `${htmlUrl}.git` as GitHub Desktop builds them).

use corvene_core::UPSTREAM_REMOTE_NAME;
use corvene_models::{GitHubRepository, Remote};

/// `helpers/github-repo-builder.ts` `gitHubRepoFixture({ owner, name })`.
fn git_hub_repo_fixture(owner: &str, name: &str) -> GitHubRepository {
    let html_url = format!("https://github.com/{owner}/{name}");
    GitHubRepository {
        endpoint: "https://api.github.com".into(),
        owner: owner.into(),
        name: name.into(),
        clone_url: format!("{html_url}.git"),
        html_url,
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

/// Stand-in for GitHub Desktop's `findUpstreamRemote(parent, remotes)`
/// (`lib/stores/helpers/find-upstream-remote.ts`).
fn find_upstream_remote(_parent: &GitHubRepository, _remotes: &[Remote]) -> Option<Remote> {
    unimplemented!("Corvene has no findUpstreamRemote (lib/stores/helpers/find-upstream-remote.ts)")
}

// GHD: unit/find-upstream-remote-test.ts › findUpstreamRemote › finds the upstream
#[test]
#[ignore = "ghd: missing: Corvene has no findUpstreamRemote (lib/stores/helpers/find-upstream-remote.ts), only UPSTREAM_REMOTE_NAME and url_matches_remote"]
fn finds_the_upstream() {
    let parent = git_hub_repo_fixture("somsubhra", "github-release-stats");
    let remotes = [Remote {
        name: "upstream".into(),
        url: "https://github.com/Somsubhra/github-release-stats.git".into(),
    }];
    let upstream = find_upstream_remote(&parent, &remotes);
    assert!(upstream.is_some());
    let upstream = upstream.unwrap();
    assert_eq!(upstream.name, UPSTREAM_REMOTE_NAME);
    assert_eq!(
        upstream.url,
        "https://github.com/Somsubhra/github-release-stats.git"
    );
}
