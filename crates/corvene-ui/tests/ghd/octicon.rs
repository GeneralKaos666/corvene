//! Port of GitHub Desktop's `app/test/unit/octicon-test.ts`
//! (`ui/octicons/repository.ts` `iconForRepository`).
//!
//! GitHub Desktop's `iconForRepository(repository)` picks the repository's
//! icon in the repository list and the toolbar: download for a clone in
//! progress, alert for a missing one, computer without a GitHub repository,
//! lock when private, fork for a fork, else repo. Corvene's icons are
//! `corvene_ui::icons::Octicon`, and the same choice is written out inline
//! in `RepositoryList::row` (`repository_list.rs`) and the toolbar's
//! repository button (`toolbar.rs`), with no function to call; the clone in
//! progress (GitHub Desktop's `CloningRepository`, Corvene's
//! `corvene_core::CloneState`) is drawn by `cloning_view.rs`.
//! [`icon_for_repository`] is a stand-in taking either kind
//! ([`RepositoryOrCloning`]).

use corvene_core::{CloneState, GitHubRepository, Repository};
use corvene_ui::icons::Octicon;

/// GitHub Desktop's `Repository | CloningRepository`.
#[allow(dead_code)] // read by the real iconForRepository
enum RepositoryOrCloning<'a> {
    Repository(&'a Repository),
    Cloning(&'a CloneState),
}

/// Stand-in for GitHub Desktop's `iconForRepository(repository)`
/// (`ui/octicons/repository.ts`). Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`s.
fn icon_for_repository(_repository: RepositoryOrCloning<'_>) -> Octicon {
    unimplemented!("Corvene has no iconForRepository (ui/octicons/repository.ts)")
}

/// GitHub Desktop's `getDotComAPIEndpoint()`.
const DOTCOM_API_ENDPOINT: &str = "https://api.github.com";

/// GitHub Desktop's `IGitHubRepoFixtureOptions`
/// (`app/test/helpers/github-repo-builder.ts`).
#[derive(Default)]
struct GitHubRepoFixtureOptions<'a> {
    owner: &'a str,
    name: &'a str,
    parent: Option<GitHubRepository>,
    is_private: Option<bool>,
    /// github.com when `None`.
    endpoint: Option<&'a str>,
}

/// GitHub Desktop's `gitHubRepoFixture(options)`: a `GitHubRepository` with
/// the html URL `<endpoint or https://github.com>/<owner>/<name>` and that
/// URL plus `.git` as the clone URL. Corvene's model has no database ids,
/// so GitHub Desktop's id counter has nothing to set. GitHub Desktop's
/// `isPrivate: null` (not given) is `private: false`, the value Corvene keeps
/// for an unknown visibility, and GitHub Desktop's `fork` is derived from the
/// parent (`!!parent`), so `fork` is set exactly when a parent is given.
fn github_repo_fixture(options: GitHubRepoFixtureOptions<'_>) -> GitHubRepository {
    let GitHubRepoFixtureOptions {
        owner,
        name,
        parent,
        is_private,
        endpoint,
    } = options;
    let html_url = format!(
        "{}/{owner}/{name}",
        endpoint.unwrap_or("https://github.com")
    );
    GitHubRepository {
        endpoint: endpoint.unwrap_or(DOTCOM_API_ENDPOINT).to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        clone_url: format!("{html_url}.git"),
        html_url,
        default_branch: None,
        private: is_private.unwrap_or(false),
        fork: parent.is_some(),
        parent: parent.map(Box::new),
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

/// `new Repository(path, id, gitHubRepository, false)`
fn repository(path: &str, id: u64, github: Option<GitHubRepository>) -> Repository {
    let mut repository = Repository::new(id, path);
    repository.github = github;
    repository.missing = false;
    repository
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows download icon for cloning repository
#[test]
#[ignore = "ghd: missing: no iconForRepository (ui/octicons/repository.ts); inlined in RepositoryList::row and toolbar.rs"]
fn shows_download_icon_for_cloning_repository() {
    // `new CloningRepository('C:/some/path/to/repo',
    // 'https://github.com/desktop/desktop')`
    let repository = CloneState {
        url: "https://github.com/desktop/desktop".to_string(),
        path: "C:/some/path/to/repo".into(),
        description: String::new(),
        value: None,
        cancel: corvene_git::CancelToken::new(),
    };
    let icon = icon_for_repository(RepositoryOrCloning::Cloning(&repository));
    assert_eq!(icon, Octicon::DesktopDownload);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows computer icon for non-GitHub repository
#[test]
#[ignore = "ghd: missing: no iconForRepository (ui/octicons/repository.ts); inlined in RepositoryList::row and toolbar.rs"]
fn shows_computer_icon_for_non_github_repository() {
    let repository = repository("C:/some/path/to/repo", 1, None);
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::DeviceDesktop);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows repo icon for public GitHub repository
#[test]
#[ignore = "ghd: missing: no iconForRepository (ui/octicons/repository.ts); inlined in RepositoryList::row and toolbar.rs"]
fn shows_repo_icon_for_public_github_repository() {
    let github_repository = github_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(false),
        ..Default::default()
    });
    let repository = repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::Repo);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows lock icon for private GitHub repository
#[test]
#[ignore = "ghd: missing: no iconForRepository (ui/octicons/repository.ts); inlined in RepositoryList::row and toolbar.rs"]
fn shows_lock_icon_for_private_github_repository() {
    let github_repository = github_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(true),
        ..Default::default()
    });
    let repository = repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::Lock);
}

// GHD: unit/octicon-test.ts › octicon/iconForRepository › shows fork icon for forked GitHub repository
#[test]
#[ignore = "ghd: missing: no iconForRepository (ui/octicons/repository.ts); inlined in RepositoryList::row and toolbar.rs"]
fn shows_fork_icon_for_forked_github_repository() {
    let github_repository = github_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "my-repo",
        is_private: Some(false),
        parent: Some(github_repo_fixture(GitHubRepoFixtureOptions {
            owner: "you",
            name: "my-repo",
            ..Default::default()
        })),
        ..Default::default()
    });
    let repository = repository("C:/some/path/to/repo", 1, Some(github_repository));
    let icon = icon_for_repository(RepositoryOrCloning::Repository(&repository));
    assert_eq!(icon, Octicon::RepoForked);
}
