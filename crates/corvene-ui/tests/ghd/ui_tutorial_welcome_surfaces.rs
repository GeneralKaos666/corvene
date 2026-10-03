//! Port of GitHub Desktop's
//! `app/test/unit/ui/tutorial-welcome-surfaces-test.tsx`.
//!
//! The onboarding tutorial's pages are GPUI elements in Corvene
//! (`crates/corvene-ui/src/tutorial_panel.rs`), drawn from the app state
//! with nothing to read back, so each case calls a stand-in returning what
//! the GitHub Desktop component shows:
//!
//! - `TutorialWelcome` (`ui/tutorial/welcome.tsx`): "Welcome to GitHub
//!   Desktop", the subtitle, three definitions with illustrations and their
//!   alternative texts. Corvene: `tutorial_panel::tutorial_welcome(cx)`,
//!   whose images have no alternative text. The product name is flag
//!   `103-product-name`, so [`tutorial_welcome`] takes it and the case
//!   passes the GitHub Desktop preset's value (read from the flag
//!   registry).
//! - `TutorialStepInstructions` (`ui/tutorial/tutorial-step-instruction.tsx`):
//!   a step's `<details>` is open when it is the open section; a completed
//!   step shows a green check, the next step to do its number
//!   (`orderedTutorialSteps` index + 1) in a blue circle, any other its
//!   number in an empty circle; the Skip link shows on the open next step.
//!   Corvene: `TutorialPanel::step` (private, inside the view) computes the
//!   same from `TutorialStep::completes` and the current step.
//!   [`tutorial_step_instructions`] takes GitHub Desktop's props
//!   (`isComplete` / `isNextStepTodo` as closures).
//! - `TutorialDone` (`ui/tutorial/done.tsx`): "You're done!", the "Hands
//!   clapping" image and three suggested actions (Open in Browser →
//!   `showGitHubExplore(repository)`, Create Repository / Add Repository →
//!   the popups). Corvene: `tutorial_panel::tutorial_done(cx)`, whose Open in
//!   Browser opens `https://github.com/explore` (GitHub Desktop opens
//!   `/explore` on the repository's host) and whose image has no
//!   alternative text. GitHub Desktop's `PopupType.CreateRepository` /
//!   `AddRepository` are `corvene_core::Popup::CreateRepository` /
//!   `AddExistingRepository` without a path.
//!
//! A click on a button is GitHub Desktop's callback prop; here the content
//! names the action the button runs, which is what the click then checks.
//! Not ported:
//!
//! - `TutorialDone` focusing its heading once (`document.activeElement`,
//!   `onTutorialCompletionAnnounced` called once): Corvene announces the
//!   page through a live region instead of focusing its heading
//!   (`.docs/deviations.md` › Tutorial, "the done page announces itself
//!   through a live region instead of focusing its heading");
//! - React DOM and event wiring: the `<li>` / `<img>` elements as such, the
//!   step's children (rendered unchanged; Corvene's `step` takes them as an
//!   element) and the `toggle` event forwarding `sectionId` to
//!   `onSummaryClick` (Corvene's summary row toggles the open step in its
//!   GPUI click handler).

use corvene_core::flags::{Preset, def, ids};
use corvene_core::tutorial::TutorialStep;
use corvene_core::{GitHubRepository, Popup, Repository};

/// Flag `103-product-name` at its GitHub Desktop preset value.
fn ghd_product_name() -> String {
    def(ids::PRODUCT_NAME)
        .value_for(Preset::GitHubDesktop)
        .as_text()
        .expect("103-product-name is a text flag")
        .to_string()
}

/// What GitHub Desktop's `TutorialWelcome` shows.
#[allow(dead_code)] // filled by the real TutorialWelcome content
struct TutorialWelcomeContent {
    /// The `<h1>`.
    title: String,
    /// The `<p>` under it.
    text: String,
    /// The definitions' texts, in order.
    definitions: Vec<String>,
    /// The illustrations' alternative texts, in order.
    image_alts: Vec<String>,
}

/// Stand-in for GitHub Desktop's `TutorialWelcome`
/// (`ui/tutorial/welcome.tsx`) naming the app `product_name` (flag
/// `103-product-name`). Replace it with the Corvene function once there is
/// one and remove the `#[ignore]`.
fn tutorial_welcome(_product_name: &str) -> TutorialWelcomeContent {
    unimplemented!(
        "Corvene has no TutorialWelcome content: tutorial_panel::tutorial_welcome returns a GPUI element"
    )
}

/// The circle a tutorial step shows before its summary.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real TutorialStepInstructions content
enum TutorialStepIcon {
    /// `.green-circle` with the check octicon: a completed step.
    GreenCheck,
    /// `.blue-circle` with the step's number: the next step to do.
    Blue(String),
    /// `.empty-circle` with the step's number.
    Empty(String),
}

/// What GitHub Desktop's `TutorialStepInstructions` shows.
#[allow(dead_code)] // filled by the real TutorialStepInstructions content
struct TutorialStepInstructionsContent {
    /// The `<details>` is open.
    open: bool,
    icon: TutorialStepIcon,
    summary_text: String,
    /// The Skip link is shown (otherwise the chevron).
    skip_link: bool,
}

/// Stand-in for GitHub Desktop's `TutorialStepInstructions`
/// (`ui/tutorial/tutorial-step-instruction.tsx`); `skip_link`: a
/// `skipLinkButton` was given. Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`.
fn tutorial_step_instructions(
    _summary_text: &str,
    _is_complete: &dyn Fn(TutorialStep) -> bool,
    _section_id: TutorialStep,
    _is_next_step_todo: &dyn Fn(TutorialStep) -> bool,
    _currently_open_section_id: TutorialStep,
    _skip_link: bool,
) -> TutorialStepInstructionsContent {
    unimplemented!(
        "Corvene has no TutorialStepInstructions content: TutorialPanel::step computes it inside the GPUI view"
    )
}

/// What a suggested action of `TutorialDone` does.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // built by the real TutorialDone content
enum TutorialDoneAction {
    /// `dispatcher.showGitHubExplore(repository)` (the repository's id).
    ShowGitHubExplore(u64),
    /// `dispatcher.showPopup(popup)`
    ShowPopup(Popup),
}

/// What GitHub Desktop's `TutorialDone` shows.
#[allow(dead_code)] // filled by the real TutorialDone content
struct TutorialDoneContent {
    /// The `<h1>`.
    heading: String,
    /// The illustration's alternative text, `None` without one.
    image_alt: Option<String>,
    /// The suggested actions' buttons: label and what each does.
    actions: Vec<(String, TutorialDoneAction)>,
}

/// Stand-in for GitHub Desktop's `TutorialDone` (`ui/tutorial/done.tsx`)
/// for the tutorial `repository`. Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`.
fn tutorial_done(_repository: &Repository) -> TutorialDoneContent {
    unimplemented!(
        "Corvene has no TutorialDone content: tutorial_panel::tutorial_done returns a GPUI element"
    )
}

/// GitHub Desktop's `createRepository()`: `octocat/desktop` on github.com
/// at `/tmp/tutorial-fixture` (no clone URL: null, an empty string here).
fn create_repository() -> Repository {
    let mut repository = Repository::new(5, "/tmp/tutorial-fixture");
    repository.github = Some(GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: "octocat".to_string(),
        name: "desktop".to_string(),
        html_url: "https://github.com/octocat/desktop".to_string(),
        clone_url: String::new(),
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    });
    repository
}

// GHD: unit/ui/tutorial-welcome-surfaces-test.tsx › tutorial welcome surfaces › renders the tutorial welcome definitions and images
#[test]
#[ignore = "ghd: missing: no TutorialWelcome content (tutorial/welcome.tsx); tutorial_panel::tutorial_welcome draws it as a GPUI element whose images have no alternative text"]
fn renders_the_tutorial_welcome_definitions_and_images() {
    let view = tutorial_welcome(&ghd_product_name());

    assert_eq!(view.title, "Welcome to GitHub Desktop");
    assert_eq!(
        view.text,
        "Use this tutorial to get comfortable with Git, GitHub, and GitHub Desktop."
    );
    assert_eq!(view.definitions.len(), 3);
    assert_eq!(
        view.image_alts,
        [
            "Html syntax icon",
            "People with discussion bubbles overhead",
            "Server stack with cloud",
        ]
    );
}

// GHD: unit/ui/tutorial-welcome-surfaces-test.tsx › tutorial welcome surfaces › renders tutorial step instructions with step state, skip content, and toggle callbacks
#[test]
#[ignore = "ghd: missing: no TutorialStepInstructions content (tutorial-step-instruction.tsx); TutorialPanel::step computes the open state, circle and Skip link inside the GPUI view"]
fn renders_tutorial_step_instructions_with_step_state_skip_content_and_toggle_callbacks() {
    let is_complete = |step: TutorialStep| step == TutorialStep::PickEditor;
    let is_next_step_todo = |step: TutorialStep| step == TutorialStep::CreateBranch;

    let view = tutorial_step_instructions(
        "Create a branch",
        &is_complete,
        TutorialStep::CreateBranch,
        &is_next_step_todo,
        TutorialStep::CreateBranch,
        true,
    );

    assert!(view.open);
    assert_eq!(view.icon, TutorialStepIcon::Blue("2".to_string()));
    assert_eq!(view.summary_text, "Create a branch");
    // `getByRole('button', { name: 'Skip' })`
    assert!(view.skip_link);

    // `view.rerender(…)` for the completed PickEditor step, no skip link
    let rerendered = tutorial_step_instructions(
        "Pick your editor",
        &is_complete,
        TutorialStep::PickEditor,
        &is_next_step_todo,
        TutorialStep::CreateBranch,
        false,
    );

    assert_eq!(rerendered.icon, TutorialStepIcon::GreenCheck);
}

// GHD: unit/ui/tutorial-welcome-surfaces-test.tsx › tutorial welcome surfaces › focuses tutorial completion once and routes suggested actions through the dispatcher
#[test]
#[ignore = "ghd: missing: no TutorialDone content (tutorial/done.tsx); tutorial_panel::tutorial_done draws it as a GPUI element (no image alternative text, Open in Browser opens github.com/explore directly)"]
fn focuses_tutorial_completion_once_and_routes_suggested_actions_through_the_dispatcher() {
    let repository = create_repository();

    let view = tutorial_done(&repository);

    let open_explore_label = if cfg!(target_os = "macos") {
        "Open in Browser"
    } else {
        "Open in browser"
    };
    let create_repository_label = if cfg!(target_os = "macos") {
        "Create Repository"
    } else {
        "Create repository"
    };
    let add_repository_label = if cfg!(target_os = "macos") {
        "Add Repository"
    } else {
        "Add repository"
    };

    // `getByRole('heading', { name: "You're done!" })`
    assert_eq!(view.heading, "You're done!");
    // `getByRole('img', { name: 'Hands clapping' })`
    assert_eq!(view.image_alt.as_deref(), Some("Hands clapping"));

    // `getByRole('button', { name })` for each label, clicked once:
    // explore for the repository, then the two popups
    let action = |label: &str| {
        view.actions
            .iter()
            .find(|(button, _)| button == label)
            .map(|(_, action)| action.clone())
    };
    assert_eq!(
        action(open_explore_label),
        Some(TutorialDoneAction::ShowGitHubExplore(repository.id))
    );
    assert_eq!(
        action(create_repository_label),
        Some(TutorialDoneAction::ShowPopup(Popup::CreateRepository {
            path: None
        }))
    );
    assert_eq!(
        action(add_repository_label),
        Some(TutorialDoneAction::ShowPopup(
            Popup::AddExistingRepository { path: None }
        ))
    );
}
