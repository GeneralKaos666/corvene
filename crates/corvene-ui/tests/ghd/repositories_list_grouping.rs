//! Port of GitHub Desktop's `app/test/unit/repositories-list-grouping-test.ts`.
//!
//! GitHub Desktop's `groupRepositories(repositories, localRepositoryStateLookup,
//! recentRepositories)` (`ui/repositories-list/group-repositories.ts`) has no
//! callable Corvene equivalent: the repository foldout groups inside
//! `RepositoryFoldout::groups` (`crates/corvene-ui/src/repository_list.rs`),
//! a private method of the gpui view that reads the app state, the filter
//! box and the flags through a gpui `App`, and returns titled groups without
//! GitHub Desktop's group kinds or per-item `needsDisambiguation` (Corvene's
//! duplicate-name detail is `duplicate_name_paths`, flag
//! `213-duplicate-names-show-path`). It also groups GitHub Enterprise
//! repositories by owner, where GitHub Desktop puts them in one
//! `enterprise` group per host. The cases call a stand-in and are ignored
//! until a pure grouping function exists.
//!
//! Types: GitHub Desktop's `Repository` is `corvene_models::Repository`
//! (`Repository::new(id, path)`, `github` for the `GitHubRepository`);
//! `ILocalRepositoryState` is `corvene_core::RepoIndicator` (the
//! foldout's ahead/behind and changed-file counts);
//! `corvene_test_support::git_hub_repo_fixture` is GitHub Desktop's
//! `helpers/github-repo-builder.ts`.

use std::collections::HashMap;
use std::path::Path;

use corvene_core::{RepoIndicator, Repository};
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture, new_repository};

/// GitHub Desktop's `RepositoryListGroup`.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum RepositoryListGroup {
    Recent,
    Other,
    /// The owner's login.
    Dotcom {
        owner: String,
    },
    Enterprise {
        host: String,
    },
}

impl RepositoryListGroup {
    /// GitHub Desktop's `identifier.kind`.
    fn kind(&self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Other => "other",
            Self::Dotcom { .. } => "dotcom",
            Self::Enterprise { .. } => "enterprise",
        }
    }

    /// GitHub Desktop's `(identifier as any).owner.login`.
    fn owner_login(&self) -> &str {
        match self {
            Self::Dotcom { owner } => owner,
            other => panic!("{other:?} has no owner"),
        }
    }
}

/// GitHub Desktop's `IRepositoryListItem`.
#[allow(dead_code)]
struct RepositoryListItem {
    text: Vec<String>,
    id: String,
    repository: Repository,
    needs_disambiguation: bool,
}

/// GitHub Desktop's `IFilterListGroup<IRepositoryListItem, RepositoryListGroup>`.
struct RepositoryGroup {
    identifier: RepositoryListGroup,
    items: Vec<RepositoryListItem>,
}

/// Stand-in for GitHub Desktop's `groupRepositories` (see the module doc).
/// Replace it with the Corvene function once the foldout's grouping is one
/// and remove the `#[ignore]`s.
fn group_repositories(
    _repositories: &[Repository],
    _local_repository_state_lookup: &HashMap<u64, RepoIndicator>,
    _recent_repositories: &[u64],
) -> Vec<RepositoryGroup> {
    unimplemented!(
        "the repository foldout's grouping is RepositoryFoldout::groups, which needs a gpui App"
    )
}

/// The `repositories` of the test's `describe`.
fn repositories() -> Vec<Repository> {
    vec![
        new_repository("repo1", 1, None),
        new_repository(
            "repo2",
            2,
            Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
                owner: "me",
                name: "my-repo2",
                ..Default::default()
            })),
        ),
        new_repository(
            "repo3",
            3,
            Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
                owner: "",
                name: "my-repo3",
                endpoint: Some("https://github.big-corp.com/api/v3"),
                ..Default::default()
            })),
        ),
    ]
}

// GHD: unit/repositories-list-grouping-test.ts › repository list grouping › groups repositories by owners/Enterprise/Other
#[test]
#[ignore = "ghd: missing: groupRepositories (ui/repositories-list/group-repositories.ts) is inside RepositoryFoldout::groups (needs a gpui App, no group kinds); Corvene also groups GHES repositories by owner, not one enterprise group per host"]
fn groups_repositories_by_owners_enterprise_other() {
    let repositories = repositories();
    let cache = HashMap::new();

    let grouped = group_repositories(&repositories, &cache, &[]);
    assert_eq!(grouped.len(), 3);

    assert_eq!(grouped[0].identifier.kind(), "dotcom");
    assert_eq!(grouped[0].identifier.owner_login(), "me");
    assert_eq!(grouped[0].items.len(), 1);

    let mut item = &grouped[0].items[0];
    assert_eq!(item.repository.path, Path::new("repo2"));

    assert_eq!(grouped[1].identifier.kind(), "enterprise");
    assert_eq!(grouped[1].items.len(), 1);

    item = &grouped[1].items[0];
    assert_eq!(item.repository.path, Path::new("repo3"));

    assert_eq!(grouped[2].identifier.kind(), "other");
    assert_eq!(grouped[2].items.len(), 1);

    item = &grouped[2].items[0];
    assert_eq!(item.repository.path, Path::new("repo1"));
}

// GHD: unit/repositories-list-grouping-test.ts › repository list grouping › sorts repositories alphabetically within each group
#[test]
#[ignore = "ghd: missing: groupRepositories (ui/repositories-list/group-repositories.ts) is inside RepositoryFoldout::groups, which needs a gpui App and returns no group kinds"]
fn sorts_repositories_alphabetically_within_each_group() {
    let cache = HashMap::new();
    let repo_a = new_repository("a", 1, None);
    let repo_b = new_repository(
        "b",
        2,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "me",
            name: "b",
            ..Default::default()
        })),
    );
    let repo_c = new_repository("c", 2, None);
    let repo_d = new_repository(
        "d",
        2,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "me",
            name: "d",
            ..Default::default()
        })),
    );
    let repo_z = new_repository("z", 3, None);

    let grouped = group_repositories(&[repo_c, repo_b, repo_z, repo_d, repo_a], &cache, &[]);
    assert_eq!(grouped.len(), 2);

    assert_eq!(grouped[0].identifier.kind(), "dotcom");
    assert_eq!(grouped[0].identifier.owner_login(), "me");
    assert_eq!(grouped[0].items.len(), 2);

    let mut items = &grouped[0].items;
    assert_eq!(items[0].repository.path, Path::new("b"));
    assert_eq!(items[1].repository.path, Path::new("d"));

    assert_eq!(grouped[1].identifier.kind(), "other");
    assert_eq!(grouped[1].items.len(), 3);

    items = &grouped[1].items;
    assert_eq!(items[0].repository.path, Path::new("a"));
    assert_eq!(items[1].repository.path, Path::new("c"));
    assert_eq!(items[2].repository.path, Path::new("z"));
}

// GHD: unit/repositories-list-grouping-test.ts › repository list grouping › only disambiguates Enterprise repositories
#[test]
#[ignore = "ghd: missing: groupRepositories (ui/repositories-list/group-repositories.ts) is inside RepositoryFoldout::groups (needs a gpui App, no needsDisambiguation); Corvene also groups GHES repositories by owner, not one enterprise group per host"]
fn only_disambiguates_enterprise_repositories() {
    let cache = HashMap::new();
    let repo_a = new_repository(
        "repo",
        1,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "user1",
            name: "repo",
            ..Default::default()
        })),
    );
    let repo_b = new_repository(
        "repo",
        2,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "user2",
            name: "repo",
            ..Default::default()
        })),
    );
    let repo_c = new_repository(
        "enterprise-repo",
        3,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "business",
            name: "enterprise-repo",
            endpoint: Some("https://ghe.io/api/v3"),
            ..Default::default()
        })),
    );
    let repo_d = new_repository(
        "enterprise-repo",
        3,
        Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
            owner: "silliness",
            name: "enterprise-repo",
            endpoint: Some("https://ghe.io/api/v3"),
            ..Default::default()
        })),
    );

    let grouped = group_repositories(&[repo_a, repo_b, repo_c, repo_d], &cache, &[]);
    assert_eq!(grouped.len(), 3);

    assert_eq!(grouped[0].identifier.kind(), "dotcom");
    assert_eq!(grouped[0].identifier.owner_login(), "user1");
    assert_eq!(grouped[0].items.len(), 1);

    assert_eq!(grouped[1].identifier.kind(), "dotcom");
    assert_eq!(grouped[1].identifier.owner_login(), "user2");
    assert_eq!(grouped[1].items.len(), 1);

    assert_eq!(grouped[2].identifier.kind(), "enterprise");
    assert_eq!(grouped[2].items.len(), 2);

    assert_eq!(grouped[0].items[0].text[0], "repo");
    assert!(!grouped[0].items[0].needs_disambiguation);

    assert_eq!(grouped[1].items[0].text[0], "repo");
    assert!(!grouped[1].items[0].needs_disambiguation);

    assert_eq!(grouped[2].items[0].text[0], "enterprise-repo");
    assert!(grouped[2].items[0].needs_disambiguation);

    assert_eq!(grouped[2].items[1].text[0], "enterprise-repo");
    assert!(grouped[2].items[1].needs_disambiguation);
}
