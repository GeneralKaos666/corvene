//! Port of GitHub Desktop's
//! `app/test/unit/ui/rulesets-and-publish-surfaces-test.tsx`.
//!
//! GitHub Desktop's `RepoRulesetsForBranchLink`
//! (`ui/repository-rules/repo-rulesets-for-branch-link.tsx`) links to
//! `<htmlURL>/rules/?ref=<encodeURIComponent('refs/heads/' + branch)>`, or
//! renders its children without a link when the repository or the branch
//! is missing. Corvene builds the same URL inline, twice, in the commit
//! form's rule warnings (`ChangesSidebar`'s ports of
//! `renderBranchProtectionsRepoRulesCommitWarning` and the rule-failure
//! warning in `crates/corvene-ui/src/changes.rs`, with
//! `corvene_core::integrations::encode_component`), which draw nothing
//! until the rules snapshot has a branch. [`repo_rulesets_for_branch_link`]
//! is a stand-in returning the link's URL, `None` for "children without a
//! link".
//!
//! GitHub Desktop's `NoRemote` (`ui/repository-settings/no-remote.tsx`) is
//! the Remote tab of Repository Settings without a remote: a call to action
//! "Publish your repository to GitHub. Need help? Learn more about remote
//! repositories." whose link opens
//! `https://help.github.com/articles/about-remote-repositories/` and whose
//! Publish button calls `onPublish`. Corvene draws it inline in
//! `RepositorySettingsDialog`'s Remote tab
//! (`crates/corvene-ui/src/dialogs/repository_settings.rs`,
//! `call_to_action`), so [`no_remote`] is a stand-in returning its
//! content. Corvene's inline link opens
//! `https://docs.github.com/en/get-started/getting-started-with-git/managing-remote-repositories`
//! instead, which `.docs/deviations.md` does not list: the faithful
//! assertion below fails on it once the stand-in is replaced.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs ([`NoRemoteAction`]), which is what
//! `fireEvent.click` then checks. Not ported (React DOM only): the
//! `.repo-rulesets-for-branch-link` class, the children's text, which the
//! component renders unchanged either way, and `NoRemote`'s
//! `.dialog-content` wrapper.

use corvene_core::GitHubRepository;

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

/// Stand-in for GitHub Desktop's `RepoRulesetsForBranchLink`: the link's
/// URL, `None` when it renders its children without a link. Replace it
/// with the Corvene function once there is one and remove the `#[ignore]`s.
fn repo_rulesets_for_branch_link(
    _repository: Option<&GitHubRepository>,
    _branch: Option<&str>,
) -> Option<String> {
    unimplemented!("changes.rs builds the rulesets URL inline in the commit form's rule warnings")
}

/// What a button of `NoRemote` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real NoRemote content
enum NoRemoteAction {
    /// `onPublish`
    Publish,
}

/// What GitHub Desktop's `NoRemote` shows.
#[allow(dead_code)] // filled by the real NoRemote content
struct NoRemoteContent {
    /// The call to action's text, the link's label included.
    message: String,
    /// The help link: its label and the URL it opens.
    help_link: (String, String),
    /// The call to action's button: its label and what it does.
    action: (String, NoRemoteAction),
}

/// Stand-in for GitHub Desktop's `NoRemote`
/// (`ui/repository-settings/no-remote.tsx`). Replace it with the Corvene
/// function once there is one and remove the `#[ignore]`.
fn no_remote() -> NoRemoteContent {
    unimplemented!(
        "Corvene has no NoRemote content: repository_settings.rs draws the Remote tab's call to action inline (GPUI)"
    )
}

// GHD: unit/ui/rulesets-and-publish-surfaces-test.tsx › rulesets and publish surfaces › renders a branch rulesets link when both repository and branch are present
#[test]
#[ignore = "ghd: missing: no RepoRulesetsForBranchLink (repo-rulesets-for-branch-link.tsx); changes.rs builds the /rules/?ref= URL inline"]
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
#[ignore = "ghd: missing: no RepoRulesetsForBranchLink (repo-rulesets-for-branch-link.tsx); changes.rs builds the /rules/?ref= URL inline"]
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
#[ignore = "ghd: missing: no NoRemote content (no-remote.tsx); repository_settings.rs draws the call to action inline, and its help link opens docs.github.com/en/get-started/getting-started-with-git/managing-remote-repositories, not GHD's help.github.com/articles/about-remote-repositories/"]
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
