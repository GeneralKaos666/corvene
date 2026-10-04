//! Port of GitHub Desktop's `app/test/unit/octicon-test.ts`
//! (`ui/octicons/repository.ts` `iconForRepository`).
//!
//! GitHub Desktop's `iconForRepository(repository)` picks the repository's
//! icon in the repository list and the toolbar: download for a clone in
//! progress, alert for a missing one, computer without a GitHub repository,
//! lock when private, fork for a fork, else repo. Corvene's is
//! `corvene_ui::icons::icon_for_repository`, taking either kind
//! (`RepositoryOrCloning`; GitHub Desktop's `CloningRepository` is
//! Corvene's `corvene_core::CloneState`).

use corvene_core::CloneState;
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture, new_repository};
use corvene_ui::icons::{Octicon, RepositoryOrCloning, icon_for_repository};

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows download icon for cloning repository
#[test]
fn shows_download_icon_for_cloning_repository() {
    // `new CloningRepository('C:/some/path/to/repo',
    // 'https://github.com/desktop/desktop')`
    let repository = CloneState::new(
        "C:/some/path/to/repo".into(),
        "https://github.com/desktop/desktop".to_string(),
    );
    let icon = icon_for_repository(RepositoryOrCloning::Cloning(&repository));
    assert_eq!(icon, Octicon::DesktopDownload);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows computer icon for non-GitHub repository
#[test]
fn shows_computer_icon_for_non_github_repository() {
    let repository = new_repository("C:/some/path/to/repo", 1, None);
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::DeviceDesktop);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows repo icon for public GitHub repository
#[test]
fn shows_repo_icon_for_public_github_repository() {
    let github_repository = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(false),
        ..Default::default()
    });
    let repository = new_repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::Repo);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows lock icon for private GitHub repository
#[test]
fn shows_lock_icon_for_private_github_repository() {
    let github_repository = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(true),
        ..Default::default()
    });
    let repository = new_repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::Lock);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows fork icon for forked GitHub repository
#[test]
fn shows_fork_icon_for_forked_github_repository() {
    let github_repository = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(false),
        parent: Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "you",
            name: "my-repo",
            ..Default::default()
        })),
        ..Default::default()
    });
    let repository = new_repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::RepoForked);
}
