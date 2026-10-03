//! Port of GitHub Desktop's `app/test/unit/name-of-test.ts`
//! (`models/repository.ts` `nameOf`).
//!
//! GitHub Desktop's `nameOf(repository)` is the GitHub repository's
//! `owner/name` when there is one, else the repository's name (the folder
//! name); it ignores the alias. It names the repository in the Change
//! Repository Alias dialog, the repository list's filter text and log
//! lines. Corvene has no such function: `Repository::name()` is the alias,
//! else the GitHub repository's bare name, else the folder name (GitHub
//! Desktop's `repository.alias ?? repository.name`), the Change Repository
//! Alias dialog shows that, and the repository list composes
//! `GitHubRepository::full_name()` inline. [`name_of`] is a stand-in.

use corvene_models::{GitHubRepository, Repository};

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

/// Stand-in for GitHub Desktop's `nameOf(repository)`
/// (`models/repository.ts`). Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`s.
fn name_of(_repository: &Repository) -> String {
    unimplemented!("Corvene has no nameOf (models/repository.ts)")
}

const REPO_PATH: &str = "/some/cool/path";

// GHD: unit/name-of-test.ts › nameOf › Returns the repo base path if there is no associated github metadata
#[test]
#[ignore = "ghd: missing: no nameOf (models/repository.ts); Repository::name() returns the alias or the bare GitHub name"]
fn returns_the_repo_base_path_if_there_is_no_associated_github_metadata() {
    // `new Repository(repoPath, -1, null, false)`: Corvene's ids are
    // unsigned, the id plays no part in the name
    let repo = Repository::new(0, REPO_PATH);

    let name = name_of(&repo);

    assert_eq!(name, "path");
}

// GHD: unit/name-of-test.ts › nameOf › Returns the name of the repo
#[test]
#[ignore = "ghd: missing: no nameOf (models/repository.ts); Repository::name() returns the alias or the bare GitHub name"]
fn returns_the_name_of_the_repo() {
    let gh_repo = github_repo_fixture(GitHubRepoFixtureOptions {
        owner: "desktop",
        name: "name",
        ..Default::default()
    });
    let mut repo = Repository::new(0, REPO_PATH);
    repo.github = Some(gh_repo);

    let name = name_of(&repo);

    assert_eq!(name, "desktop/name");
}
