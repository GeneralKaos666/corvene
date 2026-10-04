//! Port of GitHub Desktop's
//! `app/test/unit/ui/rulesets-and-publish-surfaces-test.tsx`.
//!
//! GitHub Desktop's `RepoRulesetsForBranchLink`
//! (`ui/repository-rules/repo-rulesets-for-branch-link.tsx`) links to
//! `<htmlURL>/rules/?ref=<encodeURIComponent('refs/heads/' + branch)>`, or
//! renders its children without a link when the repository or the branch
//! is missing. Corvene's counterpart is
//! `corvene_ui::changes::repo_rulesets_for_branch_link(repository, branch)`,
//! the URL the commit form's rule warnings (`ChangesSidebar`'s ports of
//! `renderBranchProtectionsRepoRulesCommitWarning` and the rule-failure
//! warning) link, `None` for "children without a link".
//!
//! GitHub Desktop's `NoRemote` (`ui/repository-settings/no-remote.tsx`) is
//! the Remote tab of Repository Settings without a remote: a call to action
//! "Publish your repository to GitHub. Need help? Learn more about remote
//! repositories." whose link opens
//! `https://help.github.com/articles/about-remote-repositories/` and whose
//! Publish button calls `onPublish`. Corvene's counterpart is
//! `corvene_ui::dialogs::no_remote()`, the content
//! `RepositorySettingsDialog`'s Remote tab
//! (`crates/corvene-ui/src/dialogs/repository_settings.rs`) draws.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs (`NoRemoteAction`), which is what
//! `fireEvent.click` then checks. Not ported (React DOM only): the
//! `.repo-rulesets-for-branch-link` class, the children's text, which the
//! component renders unchanged either way, and `NoRemote`'s
//! `.dialog-content` wrapper.

use corvene_core::GitHubRepository;
use corvene_ui::changes::repo_rulesets_for_branch_link;
use corvene_ui::dialogs::{NoRemoteAction, no_remote};

/// GitHub Desktop's `createGitHubRepository()`: `desktop/desktop` on
/// github.com with the html URL `https://github.com/desktop/desktop` (no
/// clone URL: null, an empty string here).
fn create_github_repository() -> GitHubRepository {
    GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: "desktop".to_string(),
        name: "desktop".to_string(),
        html_url: "https://github.com/desktop/desktop".to_string(),
        clone_url: String::new(),
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

// GHD: unit/ui/rulesets-and-publish-surfaces-test.tsx › rulesets and publish surfaces › renders a branch rulesets link when both repository and branch are present
#[test]
fn renders_a_branch_rulesets_link_when_both_repository_and_branch_are_present() {
    let repository = create_github_repository();

    let link = repo_rulesets_for_branch_link(Some(&repository), Some("feature/notifications"));

    assert_eq!(
        link.as_deref(),
        Some(
            "https://github.com/desktop/desktop/rules/?ref=refs%2Fheads%2Ffeature%2Fnotifications"
        )
    );
}

// GHD: unit/ui/rulesets-and-publish-surfaces-test.tsx › rulesets and publish surfaces › renders raw children instead of a link when repository or branch are missing
#[test]
fn renders_raw_children_instead_of_a_link_when_repository_or_branch_are_missing() {
    let repository = create_github_repository();

    let missing_repo = repo_rulesets_for_branch_link(None, Some("main"));
    let missing_branch = repo_rulesets_for_branch_link(Some(&repository), None);

    // no `.repo-rulesets-for-branch-link` at all
    assert_eq!(
        [missing_repo, missing_branch]
            .iter()
            .filter(|link| link.is_some())
            .count(),
        0
    );
}

// GHD: unit/ui/rulesets-and-publish-surfaces-test.tsx › rulesets and publish surfaces › renders the no-remote publish call to action and invokes publish
#[test]
fn renders_the_no_remote_publish_call_to_action_and_invokes_publish() {
    let view = no_remote();

    // `getByRole('button', { name: 'Publish' })`, clicked once
    assert_eq!(
        view.action,
        ("Publish".to_string(), NoRemoteAction::Publish)
    );
    // `getByText(…, { exact: false })`
    assert!(
        view.message
            .contains("Publish your repository to GitHub. Need help?")
    );
    assert_eq!(view.help_link.0, "Learn more about remote repositories.");
    assert_eq!(
        view.help_link.1,
        "https://help.github.com/articles/about-remote-repositories/"
    );
}
