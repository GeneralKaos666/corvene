//! Port of GitHub Desktop's `app/test/unit/repositories-list-grouping-test.ts`.
//!
//! GitHub Desktop's `groupRepositories(repositories, localRepositoryStateLookup,
//! recentRepositories)` (`ui/repositories-list/group-repositories.ts`) is
//! `corvene_ui::repository_list::group_repositories`, which the repository
//! foldout groups with (`RepositoryFoldout::groups`) before filtering.
//! [`group_repositories`] maps its groups onto the GitHub Desktop shapes
//! below (a change of representation only).
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

/// GitHub Desktop's `groupRepositories`: Corvene's, in GitHub Desktop's
/// shapes.
fn group_repositories(
    repositories: &[Repository],
    local_repository_state_lookup: &HashMap<u64, RepoIndicator>,
    recent_repositories: &[u64],
) -> Vec<RepositoryGroup> {
    use corvene_ui::repository_list::RepositoryListGroup as Group;
    corvene_ui::repository_list::group_repositories(
        repositories,
        local_repository_state_lookup,
        recent_repositories,
    )
    .into_iter()
    .map(|group| RepositoryGroup {
        identifier: match group.identifier {
            Group::Recent => RepositoryListGroup::Recent,
            Group::Other => RepositoryListGroup::Other,
            Group::Dotcom { owner } => RepositoryListGroup::Dotcom { owner },
            Group::Enterprise { host } => RepositoryListGroup::Enterprise { host },
        },
        items: group
            .items
            .into_iter()
            .map(|item| RepositoryListItem {
                text: item.text,
                id: item.id,
                repository: item.repository,
                needs_disambiguation: item.needs_disambiguation,
            })
            .collect(),
    })
    .collect()
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
