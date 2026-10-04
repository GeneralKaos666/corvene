//! Port of GitHub Desktop's `app/test/unit/repositories-clone-grouping-test.ts`.
//!
//! GitHub Desktop's `groupRepositories(repositories, login)`
//! (`ui/clone-repository/group-repositories.ts`) is
//! `corvene_ui::cloneable_repositories::group_rows(repositories, login,
//! "")` (an empty filter keeps every repository). It returns the list as
//! the clone list draws it, a group's `Header(title)` row followed by its
//! `Item` rows; [`groups`] reads that back into GitHub Desktop's
//! `{ identifier, items }` groups. The group of the user's own repositories
//! is identified by its title, `YOUR_REPOSITORIES` (GHD
//! `YourRepositoriesIdentifier`); the others by the owner's login, as in
//! GitHub Desktop.
//!
//! GitHub Desktop's `IAPIFullRepository` is `corvene_models::GitHubRepository`
//! (re-exported by `corvene_core`): `owner` is the owner's login,
//! `permissions { pull, push, admin: false }` is `RepositoryPermission::Write`,
//! `default_branch: ''` is `Some("")`, and the API endpoint (not part of
//! GitHub Desktop's object) is GitHub.com's. `pushed_at` and `has_issues`
//! have no Corvene field.

use corvene_core::{GitHubRepository, RepositoryPermission};
use corvene_ui::cloneable_repositories::{CloneRow, YOUR_REPOSITORIES, group_rows};

/// One of GitHub Desktop's `IFilterListGroup<ICloneableRepositoryListItem>`.
struct Group {
    identifier: String,
    items: Vec<GitHubRepository>,
}

/// GitHub Desktop's `groupRepositories(repositories, login)`.
fn group_repositories(repositories: &[GitHubRepository], login: &str) -> Vec<Group> {
    groups(group_rows(repositories, login, ""))
}

/// The header / item rows back as groups.
fn groups(rows: Vec<CloneRow>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for row in rows {
        match row {
            CloneRow::Header(identifier) => groups.push(Group {
                identifier,
                items: Vec::new(),
            }),
            CloneRow::Item(repository, _) => groups
                .last_mut()
                .expect("an item row before any header")
                .items
                .push(repository),
        }
    }
    groups
}

/// The `users` of the test: an owner's login.
struct Users {
    shiftkey: &'static str,
    desktop: &'static str,
    octokit: &'static str,
}

const USERS: Users = Users {
    shiftkey: "shiftkey",
    desktop: "desktop",
    octokit: "octokit",
};

fn api_repository(name: &str, owner: &str, private: bool, fork: bool) -> GitHubRepository {
    GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        html_url: String::new(),
        clone_url: String::new(),
        default_branch: Some(String::new()),
        private,
        fork,
        parent: None,
        archived: false,
        permissions: Some(RepositoryPermission::Write),
        allow_forking: None,
    }
}

// GHD: unit/repositories-clone-grouping-test.ts › clone repository grouping › groups repositories by owner
#[test]
fn groups_repositories_by_owner() {
    let repositories = vec![
        api_repository("some-repo", USERS.shiftkey, true, true),
        api_repository("octokit.net", USERS.octokit, false, false),
        api_repository("desktop", USERS.desktop, true, false),
    ];

    let grouped = group_repositories(&repositories, "shiftkey");
    assert_eq!(grouped.len(), 3);

    assert_eq!(grouped[0].identifier, YOUR_REPOSITORIES);
    assert_eq!(grouped[0].items.len(), 1);

    let mut item = &grouped[0].items[0];
    assert_eq!(item.name, "some-repo");

    assert_eq!(grouped[1].identifier, "desktop");
    assert_eq!(grouped[1].items.len(), 1);

    item = &grouped[1].items[0];
    assert_eq!(item.name, "desktop");

    assert_eq!(grouped[2].identifier, "octokit");
    assert_eq!(grouped[2].items.len(), 1);

    item = &grouped[2].items[0];
    assert_eq!(item.name, "octokit.net");
}
