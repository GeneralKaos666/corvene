//! Port of GitHub Desktop's `app/test/unit/model-type-guards-test.ts`
//! (`models/repository.ts`).
//!
//! GitHub Desktop's `Repository(path, id, gitHubRepository, missing)` is
//! `corvene_models::Repository` (`Repository::new(id, path)` with `github`
//! and `missing` set), its `GitHubRepository(name, owner, dbID, isPrivate,
//! htmlURL, cloneURL, issuesEnabled, isArchived, permissions, parent)` is
//! `corvene_models::GitHubRepository` (the owner's login and endpoint are
//! the `owner` and `endpoint` fields; `fork` is set with the parent, as
//! GitHub Desktop derives it from `parent`; Corvene has no database ids or
//! `issuesEnabled`).
//!
//! - `isRepositoryWithGitHubRepository` and
//!   `isRepositoryWithForkedGitHubRepository` are TypeScript type guards
//!   (`gitHubRepository instanceof GitHubRepository`, and a non-null
//!   `parent`). In Corvene the type is the `Option` itself: callers match
//!   `repository.github`, and a fork is a GitHub repository whose `parent` is
//!   set (`Repository::is_fork_contributing_to_parent`, the fork checks in
//!   `corvene_core::pull_requests` and the Repository Settings dialog), so
//!   the cases check those fields.
//! - `getGitHubHtmlUrl` is `Repository::non_fork_github()` (GitHub Desktop's
//!   `getNonForkGitHubRepository`) and its `html_url`.
//! - `isForkedRepositoryContributingToParent` is
//!   `Repository::is_fork_contributing_to_parent()`.

use corvene_models::{GitHubRepository, Repository};
use corvene_test_support::new_repository;

/// GitHub Desktop's `isRepositoryWithGitHubRepository(repository)`: the
/// `github` field is set.
fn is_repository_with_github_repository(repository: &Repository) -> bool {
    repository.github.is_some()
}

/// GitHub Desktop's `isRepositoryWithForkedGitHubRepository(repository)`: a
/// GitHub repository with a parent.
fn is_repository_with_forked_github_repository(repository: &Repository) -> bool {
    repository
        .github
        .as_ref()
        .is_some_and(|github| github.parent.is_some())
}

/// GitHub Desktop's `getGitHubHtmlUrl(repository)`.
fn get_github_html_url(repository: &Repository) -> Option<String> {
    repository
        .non_fork_github()
        .map(|github| github.html_url.clone())
}

/// `new GitHubRepository(name, new Owner(login, endpoint, _), _, false,
/// htmlURL, cloneURL, true, false, null, parent)`
fn github_repository(
    name: &str,
    owner: &str,
    endpoint: &str,
    html_url: &str,
    clone_url: &str,
    parent: Option<GitHubRepository>,
) -> GitHubRepository {
    GitHubRepository {
        endpoint: endpoint.to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        html_url: html_url.to_string(),
        clone_url: clone_url.to_string(),
        default_branch: None,
        private: false,
        fork: parent.is_some(),
        parent: parent.map(Box::new),
        archived: false,
        permissions: None,
        allow_forking: None,
        node_id: None,
    }
}

fn create_plain_repository() -> Repository {
    new_repository("/path/to/repo", 1, None)
}

fn create_github_repository() -> Repository {
    let gh_repo = github_repository(
        "repo",
        "owner",
        "https://api.github.com",
        "https://github.com/owner/repo",
        "https://github.com/owner/repo.git",
        None,
    );
    new_repository("/path/to/repo", 1, Some(gh_repo))
}

fn create_forked_github_repository() -> Repository {
    let parent_gh_repo = github_repository(
        "repo",
        "upstream-owner",
        "https://api.github.com",
        "https://github.com/upstream-owner/repo",
        "https://github.com/upstream-owner/repo.git",
        None,
    );
    let forked_gh_repo = github_repository(
        "repo",
        "fork-owner",
        "https://api.github.com",
        "https://github.com/fork-owner/repo",
        "https://github.com/fork-owner/repo.git",
        Some(parent_gh_repo),
    );
    new_repository("/path/to/fork", 2, Some(forked_gh_repo))
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithGitHubRepository › returns false for a plain local repository
#[test]
fn is_repository_with_github_repository_returns_false_for_a_plain_local_repository() {
    let repo = create_plain_repository();
    assert!(!is_repository_with_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithGitHubRepository › returns true for a GitHub-connected repository
#[test]
fn is_repository_with_github_repository_returns_true_for_a_github_connected_repository() {
    let repo = create_github_repository();
    assert!(is_repository_with_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithGitHubRepository › returns true for a forked GitHub repository
#[test]
fn is_repository_with_github_repository_returns_true_for_a_forked_github_repository() {
    let repo = create_forked_github_repository();
    assert!(is_repository_with_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithForkedGitHubRepository › returns false for a plain local repository
#[test]
fn is_repository_with_forked_github_repository_returns_false_for_a_plain_local_repository() {
    let repo = create_plain_repository();
    assert!(!is_repository_with_forked_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithForkedGitHubRepository › returns false for a non-forked GitHub repository
#[test]
fn is_repository_with_forked_github_repository_returns_false_for_a_non_forked_github_repository() {
    let repo = create_github_repository();
    assert!(!is_repository_with_forked_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isRepositoryWithForkedGitHubRepository › returns true for a forked GitHub repository
#[test]
fn is_repository_with_forked_github_repository_returns_true_for_a_forked_github_repository() {
    let repo = create_forked_github_repository();
    assert!(is_repository_with_forked_github_repository(&repo));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › getGitHubHtmlUrl › returns null for a plain local repository
#[test]
fn get_github_html_url_returns_null_for_a_plain_local_repository() {
    let repo = create_plain_repository();
    assert_eq!(get_github_html_url(&repo), None);
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › getGitHubHtmlUrl › returns the HTML URL for a GitHub repository
#[test]
fn get_github_html_url_returns_the_html_url_for_a_github_repository() {
    let repo = create_github_repository();
    let url = get_github_html_url(&repo);
    assert_eq!(url.as_deref(), Some("https://github.com/owner/repo"));
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isForkedRepositoryContributingToParent › returns false for a plain local repository
#[test]
fn is_forked_repository_contributing_to_parent_returns_false_for_a_plain_local_repository() {
    let repo = create_plain_repository();
    assert!(!repo.is_fork_contributing_to_parent());
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isForkedRepositoryContributingToParent › returns false for a non-forked GitHub repository
#[test]
fn is_forked_repository_contributing_to_parent_returns_false_for_a_non_forked_github_repository() {
    let repo = create_github_repository();
    assert!(!repo.is_fork_contributing_to_parent());
}

// GHD: unit/model-type-guards-test.ts › Repository type guards › isForkedRepositoryContributingToParent › returns true for a forked repository with default settings
#[test]
fn is_forked_repository_contributing_to_parent_returns_true_for_a_forked_repository_with_default_settings()
 {
    let repo = create_forked_github_repository();
    // Default fork contribution target is Parent
    assert!(repo.is_fork_contributing_to_parent());
}
