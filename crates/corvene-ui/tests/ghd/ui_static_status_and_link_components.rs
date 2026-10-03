//! Port of GitHub Desktop's
//! `app/test/unit/ui/static-status-and-link-components-test.tsx`
//! (`RepoRulesetLink`, `ForkSettingsDescription`; `ActionStatusIcon` and
//! `SegmentedItem` are skipped in `tools/ghd-tests/skips/ui2.tsv`).
//!
//! - `RepoRulesetLink` (`ui/repository-rules/repo-ruleset-link.tsx`) links
//!   to `<htmlURL>/rules/<rulesetId>`. Corvene builds that URL inline for
//!   each failed rule of the commit form's rule-failure warning
//!   (`crates/corvene-ui/src/changes.rs`, `format!("{html_url}/rules/{}",
//!   f.ruleset_id)`), so [`repo_ruleset_link`] is a stand-in.
//! - `ForkSettingsDescription`
//!   (`ui/repository-settings/fork-contribution-target-description.tsx`)
//!   lists five effects of the Fork Behavior choice, each naming the
//!   target repository (the fork itself for `Self`, its parent for
//!   `Parent`). Corvene's counterpart is
//!   `corvene_ui::dialogs::fork_settings_description(github, target, cx)`,
//!   which picks the name inline and returns a GPUI element, so
//!   [`fork_settings_description`] is a stand-in returning the items'
//!   texts. GitHub Desktop's `ForkContributionTarget.Self` is
//!   `ForkContributionTarget::Own`.
//!
//! Not ported (React DOM only): the `.repo-ruleset-link` class.

use corvene_core::{ForkContributionTarget, GitHubRepository, Repository};

fn github_repository(
    owner: &str,
    html_url: &str,
    parent: Option<GitHubRepository>,
) -> GitHubRepository {
    GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: owner.to_string(),
        name: "desktop".to_string(),
        html_url: html_url.to_string(),
        // GitHub Desktop's fixtures pass no clone URL (null)
        clone_url: String::new(),
        default_branch: None,
        private: false,
        fork: parent.is_some(),
        parent: parent.map(Box::new),
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

/// GitHub Desktop's `createForkRepository()`: `desktop/desktop`, a fork of
/// `github/desktop`, at `/tmp/desktop`.
fn create_fork_repository() -> Repository {
    let parent = github_repository("github", "https://github.com/github/desktop", None);
    let fork = github_repository(
        "desktop",
        "https://github.com/desktop/desktop",
        Some(parent),
    );

    let mut repository = Repository::new(1, "/tmp/desktop");
    repository.github = Some(fork);
    repository
}

/// Stand-in for GitHub Desktop's `RepoRulesetLink`: the link's URL.
/// Replace it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn repo_ruleset_link(_repository: &GitHubRepository, _ruleset_id: u64) -> String {
    unimplemented!("changes.rs builds the /rules/<id> URL inline for each failed rule")
}

/// Stand-in for GitHub Desktop's `ForkSettingsDescription`: the text of
/// each list item. Replace it with the Corvene function once there is one
/// and remove the `#[ignore]`.
fn fork_settings_description(
    _github: &GitHubRepository,
    _fork_contribution_target: ForkContributionTarget,
) -> Vec<String> {
    unimplemented!(
        "dialogs::fork_settings_description picks the target repository inline and returns a GPUI element"
    )
}

// GHD: unit/ui/static-status-and-link-components-test.tsx › static status and link components › renders repo ruleset links with the expected ruleset url
#[test]
#[ignore = "ghd: missing: no RepoRulesetLink (repo-ruleset-link.tsx); changes.rs builds the /rules/<id> URL inline"]
fn renders_repo_ruleset_links_with_the_expected_ruleset_url() {
    let repository = github_repository("desktop", "https://github.com/desktop/desktop", None);

    let link = repo_ruleset_link(&repository, 42);

    assert_eq!(link, "https://github.com/desktop/desktop/rules/42");
}

// GHD: unit/ui/static-status-and-link-components-test.tsx › static status and link components › renders fork settings descriptions for self and parent contribution targets
#[test]
#[ignore = "ghd: missing: no ForkSettingsDescription text (fork-contribution-target-description.tsx); dialogs::fork_settings_description returns a GPUI element"]
fn renders_fork_settings_descriptions_for_self_and_parent_contribution_targets() {
    let repository = create_fork_repository();

    // `isRepositoryWithForkedGitHubRepository(repository)`
    let github = repository
        .github
        .as_ref()
        .filter(|github| github.parent.is_some())
        .expect("Expected fork repository");
    let items = fork_settings_description(github, ForkContributionTarget::Own);

    assert_eq!(items.len(), 5);
    assert!(
        items
            .iter()
            .all(|item| item.trim().contains("desktop/desktop"))
    );

    let rerendered_items = fork_settings_description(github, ForkContributionTarget::Parent);

    assert!(
        rerendered_items
            .iter()
            .all(|item| item.trim().contains("github/desktop"))
    );
}
