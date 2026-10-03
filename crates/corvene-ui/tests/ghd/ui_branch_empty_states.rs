//! Port of GitHub Desktop's `app/test/unit/ui/branch-empty-states-test.tsx`.
//!
//! GitHub Desktop's empty states are React components chosen from their
//! props:
//!
//! - `NoBranches` (`ui/branches/no-branches.tsx`): with
//!   `canCreateNewBranch` the blank-slate image, "Sorry, I can't find that
//!   branch", "Do you want to create a new branch instead?", a Create New
//!   Branch button and a ProTip naming the shortcut (⌘⇧N on macOS, Ctrl+Shift+N
//!   elsewhere); otherwise `noBranchesMessage` alone.
//! - `NoPullRequests` (`ui/branches/no-pull-requests.tsx`): a title by
//!   `isSearch` / `isLoadingPullRequests`, "No open pull requests in
//!   <repository>" when neither, and a call to action: the loading line, or a
//!   "create a new branch" (on the default branch) / "create a pull request"
//!   link button.
//!
//! Corvene draws the same copy inline in GPUI elements that need a window:
//! `BranchFoldout::no_branches` (`corvene_ui::branch_list`, create variant
//! only, and always with ⌘⇧N) and `corvene_ui::pull_request_list::
//! no_pull_requests(id, repository_name, is_search, loading,
//! on_default_branch, cx)`. Other lists draw their own no-create messages
//! inline (the compare list, the Open Pull Request base list). There is no
//! function returning the content, so [`no_branches`] and
//! [`no_pull_requests`] are stand-ins returning what the components show;
//! replace them with the Corvene functions once there are some and remove
//! the `#[ignore]`s.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs ([`NoBranchesAction`],
//! [`NoPullRequestsAction`]), which is what `fireEvent.click` then checks.

/// What a `NoBranches` button does (`onCreateNewBranch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoBranchesAction {
    CreateNewBranch,
}

/// What GitHub Desktop's `NoBranches` shows.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `CreateBranch`: built by the real NoBranches content
enum NoBranchesContent {
    /// `canCreateNewBranch`: whether the blank-slate image is drawn,
    /// `.title`, `.subtitle`, the create button (its label and action) and
    /// the `.protip` text (`textContent`, the shortcut's keys included).
    CreateBranch {
        blankslate_image: bool,
        title: String,
        subtitle: String,
        button: (String, NoBranchesAction),
        protip: String,
    },
    /// Otherwise: `noBranchesMessage`, or "Sorry, I can't find that branch".
    Message(String),
}

/// Stand-in for GitHub Desktop's `NoBranches` (`ui/branches/no-branches.tsx`)
/// rendered with `canCreateNewBranch` and `noBranchesMessage`.
fn no_branches(
    _can_create_new_branch: bool,
    _no_branches_message: Option<&str>,
) -> NoBranchesContent {
    unimplemented!(
        "Corvene has no NoBranches content: BranchFoldout::no_branches draws it inline (GPUI)"
    )
}

/// What a `NoPullRequests` link button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoPullRequestsAction {
    /// `onCreateBranch`
    CreateBranch,
    /// `onCreatePullRequest`
    CreatePullRequest,
}

/// `NoPullRequests.renderCallToAction`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `Link`: built by the real NoPullRequests content
enum NoPullRequestsCallToAction {
    /// The `.call-to-action` text alone.
    Text(String),
    /// "Would you like to <link button> …": the link button's label and what
    /// it does.
    Link(String, NoPullRequestsAction),
}

/// What GitHub Desktop's `NoPullRequests` shows.
#[derive(Clone, Debug, PartialEq, Eq)]
struct NoPullRequestsContent {
    /// Whether the `.blankslate-image` is drawn.
    blankslate_image: bool,
    /// `.title`
    title: String,
    /// `.no-prs`: its text and the repository name (`<Ref>`), when neither
    /// searching nor loading.
    no_prs: Option<(String, String)>,
    call_to_action: NoPullRequestsCallToAction,
}

/// GitHub Desktop's `INoPullRequestsProps` (the callbacks are the actions).
#[allow(dead_code)] // read by the real NoPullRequests content
struct NoPullRequestsProps<'a> {
    repository_name: &'a str,
    is_on_default_branch: bool,
    is_search: bool,
    is_loading_pull_requests: bool,
}

/// Stand-in for GitHub Desktop's `NoPullRequests`
/// (`ui/branches/no-pull-requests.tsx`).
fn no_pull_requests(_props: NoPullRequestsProps<'_>) -> NoPullRequestsContent {
    unimplemented!(
        "Corvene has no NoPullRequests content: pull_request_list::no_pull_requests draws it inline (GPUI)"
    )
}

/// GitHub Desktop's `renderNoBranches(props)`: `canCreateNewBranch` true
/// unless overridden.
fn render_no_branches(
    can_create_new_branch: Option<bool>,
    no_branches_message: Option<&str>,
) -> NoBranchesContent {
    no_branches(can_create_new_branch.unwrap_or(true), no_branches_message)
}

fn create_new_branch_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Create New Branch"
    } else {
        "Create new branch"
    }
}

// GHD: unit/ui/branch-empty-states-test.tsx › branch empty states › renders the create-branch empty state and invokes the create callback
#[test]
#[ignore = "ghd: missing: no NoBranches content fn (ui/branches/no-branches.tsx); BranchFoldout::no_branches draws it inline in GPUI, with ⌘⇧N on every platform (GHD: Ctrl+Shift+N off macOS)"]
fn renders_the_create_branch_empty_state_and_invokes_the_create_callback() {
    let content = render_no_branches(None, None);

    let NoBranchesContent::CreateBranch {
        blankslate_image,
        title,
        subtitle,
        button,
        protip,
    } = content
    else {
        panic!("expected the create-branch empty state, got {content:?}");
    };

    assert_eq!(button.0, create_new_branch_label());
    // `.no-branches .blankslate-image` is not null
    assert!(blankslate_image);
    assert_eq!(title, "Sorry, I can't find that branch");
    assert_eq!(subtitle, "Do you want to create a new branch instead?");
    assert!(protip.contains(if cfg!(target_os = "macos") {
        "⌘⇧N"
    } else {
        "Ctrl+Shift+N"
    }));

    // `fireEvent.click(button)` → `createCalls.count === 1`
    assert_eq!(button.1, NoBranchesAction::CreateNewBranch);
}

// GHD: unit/ui/branch-empty-states-test.tsx › branch empty states › renders the no-create fallback message when branch creation is unavailable
#[test]
#[ignore = "ghd: missing: no NoBranches content fn (ui/branches/no-branches.tsx); Corvene has no no-create variant, each list draws its own empty message inline in GPUI"]
fn renders_the_no_create_fallback_message_when_branch_creation_is_unavailable() {
    let content = render_no_branches(Some(false), Some("No matching branches were found."));

    // `getByText(message)` and no Create New Branch button: the message
    // variant has no button
    assert_eq!(
        content,
        NoBranchesContent::Message("No matching branches were found.".to_string())
    );
}

// GHD: unit/ui/branch-empty-states-test.tsx › branch empty states › renders the search and loading pull-request placeholders
#[test]
#[ignore = "ghd: missing: no NoPullRequests content fn (ui/branches/no-pull-requests.tsx); pull_request_list::no_pull_requests chooses the copy inline in a GPUI element"]
fn renders_the_search_and_loading_pull_request_placeholders() {
    let view = no_pull_requests(NoPullRequestsProps {
        repository_name: "desktop",
        is_on_default_branch: true,
        is_search: true,
        is_loading_pull_requests: false,
    });

    assert_eq!(view.title, "Sorry, I can't find that pull request!");

    // `view.rerender(… isSearch={false} isLoadingPullRequests={true} …)`
    let view = no_pull_requests(NoPullRequestsProps {
        repository_name: "desktop",
        is_on_default_branch: true,
        is_search: false,
        is_loading_pull_requests: true,
    });

    assert_eq!(view.title, "Hang tight");
    assert_eq!(
        view.call_to_action,
        NoPullRequestsCallToAction::Text("Loading pull requests as fast as I can!".to_string())
    );
}

// GHD: unit/ui/branch-empty-states-test.tsx › branch empty states › renders default-branch and feature-branch calls to action and invokes their callbacks
#[test]
#[ignore = "ghd: missing: no NoPullRequests content fn (ui/branches/no-pull-requests.tsx); pull_request_list::no_pull_requests chooses the copy inline in a GPUI element"]
fn renders_default_branch_and_feature_branch_calls_to_action_and_invokes_their_callbacks() {
    let view = no_pull_requests(NoPullRequestsProps {
        repository_name: "desktop",
        is_on_default_branch: true,
        is_search: false,
        is_loading_pull_requests: false,
    });

    assert_eq!(view.title, "You're all set!");
    // `getByText('No open pull requests in')` and `getByText('desktop')`
    assert_eq!(
        view.no_prs,
        Some((
            "No open pull requests in".to_string(),
            "desktop".to_string()
        ))
    );
    // `.no-pull-requests .blankslate-image` is not null
    assert!(view.blankslate_image);

    let NoPullRequestsCallToAction::Link(label, action) = &view.call_to_action else {
        panic!(
            "expected a call to action with a link button, got {:?}",
            view.call_to_action
        );
    };
    assert_eq!(label, "create a new branch");
    // `fireEvent.click(createBranchButton)` → `createBranchCalls.count === 1`
    assert_eq!(*action, NoPullRequestsAction::CreateBranch);

    // `view.rerender(… isOnDefaultBranch={false} …)`
    let view = no_pull_requests(NoPullRequestsProps {
        repository_name: "desktop",
        is_on_default_branch: false,
        is_search: false,
        is_loading_pull_requests: false,
    });

    let NoPullRequestsCallToAction::Link(label, action) = &view.call_to_action else {
        panic!(
            "expected a call to action with a link button, got {:?}",
            view.call_to_action
        );
    };
    assert_eq!(label, "create a pull request");
    // `fireEvent.click(createPullRequestButton)` → `createPullRequestCalls.count === 1`
    assert_eq!(*action, NoPullRequestsAction::CreatePullRequest);
}
