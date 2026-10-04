//! Port of GitHub Desktop's `app/test/unit/find-upstream-remote-test.ts`.
//!
//! GitHub Desktop's `findUpstreamRemote(parent, remotes)`
//! (`lib/stores/helpers/find-upstream-remote.ts`) is the remote named
//! `UpstreamRemoteName` (`upstream`) when it points at the fork's parent
//! (`repositoryMatchesRemote`), else `null`; `addUpstreamRemoteIfNeeded`
//! asks it first. Corvene's is `corvene_core::forks::find_upstream_remote`
//! (with `corvene_core::UPSTREAM_REMOTE_NAME`), which
//! `Dispatcher::add_upstream_remote_if_needed` asks the same way.
//!
//! `helpers/github-repo-builder.ts` `gitHubRepoFixture` is
//! `corvene_test_support::git_hub_repo_fixture`.

use corvene_core::UPSTREAM_REMOTE_NAME;
use corvene_core::forks::find_upstream_remote;
use corvene_models::Remote;
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture};

// GHD: unit/find-upstream-remote-test.ts › findUpstreamRemote › finds the upstream
#[test]
fn finds_the_upstream() {
    let parent = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "somsubhra",
        name: "github-release-stats",
        ..Default::default()
    });
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
