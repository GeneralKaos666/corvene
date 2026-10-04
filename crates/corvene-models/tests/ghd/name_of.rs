//! Port of GitHub Desktop's `app/test/unit/name-of-test.ts`
//! (`models/repository.ts` `nameOf`).
//!
//! GitHub Desktop's `nameOf(repository)` is the GitHub repository's
//! `owner/name` when there is one, else the repository's name (the folder
//! name); it ignores the alias. It names the repository in the Change
//! Repository Alias dialog, the repository list's filter text and log
//! lines. Corvene's is [`corvene_models::name_of`] (`Repository::name()` is
//! GitHub Desktop's `repository.alias ?? repository.name`).

use corvene_models::{Repository, name_of};
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture};

const REPO_PATH: &str = "/some/cool/path";

// GHD: unit/name-of-test.ts › nameOf › Returns the repo base path if there is no associated github metadata
#[test]
fn returns_the_repo_base_path_if_there_is_no_associated_github_metadata() {
    // `new Repository(repoPath, -1, null, false)`: Corvene's ids are
    // unsigned, the id plays no part in the name
    let repo = Repository::new(0, REPO_PATH);

    let name = name_of(&repo);

    assert_eq!(name, "path");
}

// GHD: unit/name-of-test.ts › nameOf › Returns the name of the repo
#[test]
fn returns_the_name_of_the_repo() {
    let gh_repo = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "desktop",
        name: "name",
        ..Default::default()
    });
    let mut repo = Repository::new(0, REPO_PATH);
    repo.github = Some(gh_repo);

    let name = name_of(&repo);

    assert_eq!(name, "desktop/name");
}
