//! Port of GitHub Desktop's
//! `app/test/unit/ui/repository-list-item-test.tsx`.
//!
//! GitHub Desktop's `RepositoryListItem`
//! (`ui/repositories-list/repository-list-item.tsx`) shows the owner prefix
//! (`needsDisambiguation`) and the alias or name, the ahead / behind arrows
//! and the changes dot (`renderRepoIndicators`), and a tooltip with the
//! GitHub full name, the alias and the path (`renderTooltip`). Corvene's
//! row is `RepositoryFoldout::row` (`crates/corvene-ui/src/repository_list.rs`),
//! a private method of the gpui view:
//!
//! - the name is `Repository::name()` (alias, else the GitHub name, else
//!   the folder: GitHub Desktop's `alias ?? repository.name`), ported;
//! - there is no owner prefix: Corvene has no `needsDisambiguation` (its
//!   duplicate-name detail is `213-duplicate-names-show-path`, off in the
//!   GitHub Desktop preset), so [`repository_list_item_name`] is a stand-in;
//! - the indicators and the tooltip text are built inline in `row` from the
//!   app state, so [`render_repo_indicators`] and
//!   [`repository_list_item_tooltip`] are stand-ins.
//!
//! The first case is split into a ported name test and an ignored
//! indicators test. Not ported (React DOM only): the hover and tooltip
//! timers (`mouseEnter`, `advanceTimersBy(400)`), which only open the
//! tooltip whose content the third case checks.
//!
//! Types: GitHub Desktop's `Repository` is `corvene_core::Repository`
//! (`Repository::new(id, path)`); `GitHubRepository('desktop', owner, 99)`
//! has no html or clone URL (null), which Corvene's `String`s hold as empty
//! strings; `IAheadBehind` is `corvene_core::AheadBehind`.

use corvene_core::{AheadBehind, GitHubRepository, Repository};
use corvene_ui::icons::Octicon;

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

/// What GitHub Desktop's `RepositoryListItem` renders in its `.name`
/// element.
#[allow(dead_code)] // filled by the real RepositoryListItem
struct RepositoryListItemName {
    /// The `.prefix` span (`<owner>/`), when there is one.
    prefix: Option<String>,
    /// The whole `.name` element's text: the prefix, then the alias or name.
    text: String,
}

/// Stand-in for the name GitHub Desktop's `RepositoryListItem` renders.
/// Replace it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn repository_list_item_name(
    _repository: &Repository,
    _needs_disambiguation: bool,
) -> RepositoryListItemName {
    unimplemented!(
        "Corvene has no needsDisambiguation: RepositoryFoldout::row draws Repository::name() with no owner prefix"
    )
}

/// What GitHub Desktop's `renderRepoIndicators` renders.
#[allow(dead_code)] // filled by the real renderRepoIndicators
struct RepoIndicators {
    /// The `.ahead-behind` element's arrows, `None` when it is not
    /// rendered.
    ahead_behind: Option<Vec<Octicon>>,
    /// Whether the `.change-indicator-wrapper` dot is rendered.
    changes: bool,
}

/// Stand-in for GitHub Desktop's `renderRepoIndicators` with
/// `RepositoryListItem`'s `hasChanges` (`changedFilesCount > 0`). Replace
/// it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn render_repo_indicators(
    _ahead_behind: Option<AheadBehind>,
    _changed_files_count: usize,
) -> RepoIndicators {
    unimplemented!("RepositoryFoldout::row builds the indicators inline from the app state")
}

/// What GitHub Desktop's `RepositoryListItem.renderTooltip` renders.
#[allow(dead_code)] // filled by the real renderTooltip
struct RepositoryListItemTooltip {
    /// The `<strong>` GitHub full name (or name).
    full_name: String,
    /// The alias, in parentheses after it.
    alias: Option<String>,
    /// The second line.
    path: String,
}

/// Stand-in for GitHub Desktop's `RepositoryListItem.renderTooltip`.
/// Replace it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn repository_list_item_tooltip(_repository: &Repository) -> RepositoryListItemTooltip {
    unimplemented!("RepositoryFoldout::row builds the tooltip text inline")
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
#[ignore = "ghd: missing: no renderRepoIndicators (repository-list-item.tsx); RepositoryFoldout::row builds the ahead/behind arrows and changes dot inline"]
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
#[ignore = "ghd: missing: no needsDisambiguation owner prefix (repository-list-item.tsx); RepositoryFoldout::row draws Repository::name() alone"]
fn renders_owner_prefix_and_alias_when_disambiguation_is_required() {
    let repository = create_repository(Some("desktop-app"));

    let name = repository_list_item_name(&repository, true);

    assert_eq!(name.prefix.as_deref(), Some("octocat/"));
    assert_eq!(name.text, "octocat/desktop-app");
}

// GHD: unit/ui/repository-list-item-test.tsx › RepositoryListItem › shows tooltip content for the repository full name, alias, and path
#[test]
#[ignore = "ghd: missing: no renderTooltip (repository-list-item.tsx); RepositoryFoldout::row builds the tooltip text inline"]
fn shows_tooltip_content_for_the_repository_full_name_alias_and_path() {
    let repository = create_repository(Some("desktop-app"));

    let tooltip = repository_list_item_tooltip(&repository);

    assert_eq!(tooltip.full_name, "octocat/desktop");
    assert_eq!(tooltip.path, FIXTURE_REPOSITORY_PATH);
}
