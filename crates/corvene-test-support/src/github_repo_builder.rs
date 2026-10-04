//! Port of `app/test/helpers/github-repo-builder.ts`, and the `Repository`
//! model constructor the tests pair it with.

use std::path::PathBuf;

use corvene_models::{GitHubRepository, Repository};

/// GitHub Desktop's `getDotComAPIEndpoint()` (`lib/api.ts`): the API base
/// of a github.com repository's owner.
pub const DOT_COM_API_ENDPOINT: &str = "https://api.github.com";

/// GitHub Desktop's `IGitHubRepoFixtureOptions`: [`git_hub_repo_fixture`]'s
/// argument. Build it as GitHub Desktop's tests build the object literal,
/// naming only what the case gives:
///
/// ```ignore
/// git_hub_repo_fixture(GitHubRepoFixtureOptions {
///     owner: "desktop",
///     name: "desktop",
///     ..Default::default()
/// })
/// ```
#[derive(Clone, Debug, Default)]
pub struct GitHubRepoFixtureOptions<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub parent: Option<GitHubRepository>,
    /// GitHub Desktop's `isPrivate`; `None` (not given) is GitHub Desktop's
    /// `null`.
    pub is_private: Option<bool>,
    /// github.com when `None`.
    pub endpoint: Option<&'a str>,
}

/// GitHub Desktop's `gitHubRepoFixture(options)`: a `GitHubRepository` whose
/// HTML URL is `<endpoint or https://github.com>/<owner>/<name>`, whose clone
/// URL is that plus `.git`, and whose owner's endpoint is `endpoint` or
/// [`DOT_COM_API_ENDPOINT`].
///
/// Corvene's model has no database ids, so GitHub Desktop's id counter has
/// nothing to set. `isPrivate: null` (not given) is `private: false`, the
/// value Corvene keeps for an unknown visibility. GitHub Desktop derives
/// `fork` from the parent (`!!parent`), so `fork` is set exactly when a
/// parent is given. No default branch, not archived, no permissions.
pub fn git_hub_repo_fixture(options: GitHubRepoFixtureOptions<'_>) -> GitHubRepository {
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
        endpoint: endpoint.unwrap_or(DOT_COM_API_ENDPOINT).to_string(),
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

/// GitHub Desktop's `new Repository(path, id, gitHubRepository, false)`
/// (`models/repository.ts`): a repository that is not missing, with the
/// given GitHub repository (`null` is `None`).
pub fn new_repository(
    path: impl Into<PathBuf>,
    id: u64,
    git_hub_repository: Option<GitHubRepository>,
) -> Repository {
    let mut repository = Repository::new(id, path);
    repository.github = git_hub_repository;
    repository.missing = false;
    repository
}
