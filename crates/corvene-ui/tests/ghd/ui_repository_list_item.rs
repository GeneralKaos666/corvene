//! Port of GitHub Desktop's
//! `app/test/unit/ui/repository-list-item-test.tsx`.
//!
//! GitHub Desktop's `RepositoryListItem`
//! (`ui/repositories-list/repository-list-item.tsx`) shows the owner prefix
//! (`needsDisambiguation`) and the alias or name, the ahead / behind arrows
//! and the changes dot (`renderRepoIndicators`), and a tooltip with the
//! GitHub full name, the alias and the path (`renderTooltip`). Corvene's
//! row is `RepositoryFoldout::row` (`crates/corvene-ui/src/repository_list.rs`),
//! which draws `repository_list::repository_list_item_name`,
//! `render_repo_indicators` and `repository_list_item_tooltip`; the name
//! alone is `Repository::name()` (alias, else the GitHub name, else the
//! folder: GitHub Desktop's `alias ?? repository.name`).
//!
//! The first case is split into a name test and an indicators test. Not
//! ported (React DOM only): the hover and tooltip timers (`mouseEnter`,
//! `advanceTimersBy(400)`), which only open the tooltip whose content the
//! third case checks.
//!
//! Types: GitHub Desktop's `Repository` is `corvene_core::Repository`
//! (`Repository::new(id, path)`); `GitHubRepository('desktop', owner, 99)`
//! has no html or clone URL (null), which Corvene's `String`s hold as empty
//! strings; `IAheadBehind` is `corvene_core::AheadBehind`.

use corvene_core::{AheadBehind, GitHubRepository, Repository};
use corvene_ui::repository_list::{
    render_repo_indicators, repository_list_item_name, repository_list_item_tooltip,
};

const FIXTURE_REPOSITORY_PATH: &str = "/tmp/desktop-fixture";

/// GitHub Desktop's `createRepository(alias)`.
fn create_repository(alias: Option<&str>) -> Repository {
    let mut repository = Repository::new(123, FIXTURE_REPOSITORY_PATH);
    repository.github = Some(GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: "octocat".to_string(),
        name: "desktop".to_string(),
        html_url: String::new(),
        clone_url: String::new(),
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    });
    repository.alias = alias.map(str::to_string);
    repository
}

// GHD: unit/ui/repository-list-item-test.tsx › RepositoryListItem › renders the repository name and status indicators
#[test]
fn renders_the_repository_name_and_status_indicators() {
    let repository = create_repository(None);

    // `needsDisambiguation={false}`: the `.name` text is the name alone
    assert_eq!(repository.name(), "desktop");
}

// GHD: unit/ui/repository-list-item-test.tsx › RepositoryListItem › renders the repository name and status indicators
#[test]
fn renders_the_repository_name_and_status_indicators_indicators() {
    let indicators = render_repo_indicators(
        Some(AheadBehind {
            ahead: 2,
            behind: 1,
        }),
        3,
    );

    let ahead_behind = indicators.ahead_behind;
    assert!(ahead_behind.is_some());
    assert!(indicators.changes);
    assert_eq!(ahead_behind.map(|arrows| arrows.len()), Some(2));
}

// GHD: unit/ui/repository-list-item-test.tsx › RepositoryListItem › renders owner prefix and alias when disambiguation is required
#[test]
fn renders_owner_prefix_and_alias_when_disambiguation_is_required() {
    let repository = create_repository(Some("desktop-app"));

    let name = repository_list_item_name(&repository, true);

    assert_eq!(name.prefix.as_deref(), Some("octocat/"));
    assert_eq!(name.text, "octocat/desktop-app");
}

// GHD: unit/ui/repository-list-item-test.tsx › RepositoryListItem › shows tooltip content for the repository full name, alias, and path
#[test]
fn shows_tooltip_content_for_the_repository_full_name_alias_and_path() {
    let repository = create_repository(Some("desktop-app"));

    let tooltip = repository_list_item_tooltip(&repository);

    assert_eq!(tooltip.full_name, "octocat/desktop");
    assert_eq!(tooltip.path, FIXTURE_REPOSITORY_PATH);
}
