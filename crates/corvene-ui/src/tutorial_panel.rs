//! Onboarding tutorial views - GHD `ui/tutorial/tutorial-panel.tsx`,
//! `tutorial-step-instruction.tsx`, `welcome.tsx`, `done.tsx`
//! (`styles/ui/onboarding-tutorial/_right-panel.scss`, `_welcome.scss`,
//! `_done.scss`).
//!
//! While the tutorial repository is selected the right-hand panel lists the
//! steps (one expanded at a time, completed ones checked, the next one in
//! blue) with Exit Tutorial at the bottom; the Changes pane shows the welcome
//! page instead of "No local changes", and "You're done!" once every step
//! is complete.
//!
//! Deviations: the nudge arrows GHD draws on the branch and push buttons
//! and the commit button are not ported; the panel is a fixed 350 px (GHD
//! shrinks it down to 274 px on narrow windows); the done page announces
//! itself through a live region instead of moving focus to its heading.

use std::path::PathBuf;

use corvene_core::tutorial::TutorialStep;
use corvene_core::{AppState, Dispatcher, Popup, PreferencesTab};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, c, primer};
use crate::widgets::{Inline, ListRowA11y, button, code_ref, kbd_group, link_button, paragraph};

/// `.tutorial-panel-component { flex: 1 1 350px; max-width: 350px }`
#[allow(non_snake_case)]
pub fn TUTORIAL_PANEL_WIDTH() -> Pixels {
    zpx(350.)
}
/// `--spacing-triple`, `--spacing-quad`, `--spacing-quint`
#[allow(non_snake_case)]
fn SPACING_TRIPLE() -> Pixels {
    zpx(30.)
}
#[allow(non_snake_case)]
fn SPACING_QUAD() -> Pixels {
    zpx(40.)
}
#[allow(non_snake_case)]
fn SPACING_QUINT() -> Pixels {
    zpx(50.)
}
/// `--font-size-xl`
#[allow(non_snake_case)]
fn FONT_SIZE_XL() -> Pixels {
    zpx(32.)
}

/// GHD suggests Visual Studio Code (`suggestedExternalEditor`) or Atom.
const SUGGESTED_EDITOR: (&str, &str) = ("Visual Studio Code", "https://code.visualstudio.com");

/// The circle a tutorial step shows before its summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TutorialStepIcon {
    /// `.green-circle` with the check octicon: a completed step.
    GreenCheck,
    /// `.blue-circle` with the step's number: the next step to do.
    Blue(String),
    /// `.empty-circle` with the step's number.
    Empty(String),
}

/// What GHD `TutorialStepInstructions`
/// (`ui/tutorial/tutorial-step-instruction.tsx`) shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TutorialStepInstructionsContent {
    /// The `<details>` is open.
    pub open: bool,
    pub icon: TutorialStepIcon,
    pub summary_text: String,
    /// The Skip link is shown (otherwise the chevron).
    pub skip_link: bool,
}

/// GHD `TutorialStepInstructions` for `section_id`: open when it is the
/// open section (`None`: every step collapsed), the icon from `is_complete`
/// / `is_next_step_todo` and the step's `orderedTutorialSteps` number, and
/// the Skip link (`has_skip_link`: a `skipLinkButton` was given) on the
/// open next step.
pub fn tutorial_step_instructions(
    summary_text: &str,
    is_complete: &dyn Fn(TutorialStep) -> bool,
    section_id: TutorialStep,
    is_next_step_todo: &dyn Fn(TutorialStep) -> bool,
    currently_open_section_id: impl Into<Option<TutorialStep>>,
    has_skip_link: bool,
) -> TutorialStepInstructionsContent {
    let open = currently_open_section_id.into() == Some(section_id);
    let next = is_next_step_todo(section_id);
    // "ugh zero-indexing"
    let number = section_id.index().map_or(0, |ix| ix + 1).to_string();
    let icon = if is_complete(section_id) {
        TutorialStepIcon::GreenCheck
    } else if next {
        TutorialStepIcon::Blue(number)
    } else {
        TutorialStepIcon::Empty(number)
    };
    TutorialStepInstructionsContent {
        open,
        icon,
        summary_text: summary_text.to_string(),
        skip_link: has_skip_link && open && next,
    }
}

/// One of `TutorialWelcome`'s definitions: its illustration, the bold term
/// and the rest of the sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TutorialDefinition {
    pub image: &'static str,
    pub term: String,
    pub rest: &'static str,
}

impl TutorialDefinition {
    /// The definition's text.
    pub fn text(&self) -> String {
        format!("{}{}", self.term, self.rest)
    }
}

/// What GHD `TutorialWelcome` (`ui/tutorial/welcome.tsx`) shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TutorialWelcomeContent {
    /// The `<h1>`.
    pub title: String,
    /// The `<p>` under it.
    pub text: String,
    /// The definitions, in order.
    pub definitions: Vec<TutorialDefinition>,
    /// The illustrations' alternative texts, in order.
    pub image_alts: Vec<String>,
}

/// GHD `TutorialWelcome` naming the app `product_name` (flag
/// `103-product-name`; GHD: "GitHub Desktop").
pub fn tutorial_welcome_content(product_name: &str) -> TutorialWelcomeContent {
    TutorialWelcomeContent {
        title: format!("Welcome to {product_name}"),
        text: format!("Use this tutorial to get comfortable with Git, GitHub, and {product_name}."),
        definitions: vec![
            TutorialDefinition {
                image: "illustrations/code.svg",
                term: "Git".into(),
                rest: " is the version control system.",
            },
            TutorialDefinition {
                image: "illustrations/github-for-teams.svg",
                term: "GitHub".into(),
                rest: " is where you store your code and collaborate with others.",
            },
            TutorialDefinition {
                image: "illustrations/github-for-business.svg",
                term: product_name.into(),
                rest: " helps you work with GitHub locally.",
            },
        ],
        image_alts: vec![
            "Html syntax icon".into(),
            "People with discussion bubbles overhead".into(),
            "Server stack with cloud".into(),
        ],
    }
}

/// What a suggested action of `TutorialDone` does.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // built once per render
pub enum TutorialDoneAction {
    /// `dispatcher.showGitHubExplore(repository)` (the repository's id).
    ShowGitHubExplore(u64),
    /// `dispatcher.showPopup(popup)`
    ShowPopup(Popup),
}

/// What GHD `TutorialDone` (`ui/tutorial/done.tsx`) shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TutorialDoneContent {
    /// The `<h1>`.
    pub heading: String,
    /// The illustration's alternative text, `None` without one.
    pub image_alt: Option<String>,
    /// The suggested actions' buttons: label and what each does.
    pub actions: Vec<(String, TutorialDoneAction)>,
}

/// GHD `TutorialDone` for the tutorial `repository`.
pub fn tutorial_done_content(repository: &corvene_core::Repository) -> TutorialDoneContent {
    TutorialDoneContent {
        heading: "You're done!".into(),
        image_alt: Some("Hands clapping".into()),
        actions: vec![
            (
                mac_or("Open in Browser", "Open in browser").into(),
                TutorialDoneAction::ShowGitHubExplore(repository.id),
            ),
            (
                mac_or("Create Repository", "Create repository").into(),
                TutorialDoneAction::ShowPopup(Popup::CreateRepository { path: None }),
            ),
            (
                mac_or("Add Repository", "Add repository").into(),
                TutorialDoneAction::ShowPopup(Popup::AddExistingRepository { path: None }),
            ),
        ],
    }
}

/// `TutorialPanel`
pub struct TutorialPanel {
    state: Entity<AppState>,
    /// `currentlyOpenSectionId` (`None` when the open step was collapsed).
    open: Option<TutorialStep>,
    /// The step `open` was last synced to (`componentWillReceiveProps`).
    synced: Option<TutorialStep>,
}

impl TutorialPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            open: None,
            synced: None,
        }
    }

    /// `TutorialStepInstructions`: the summary row and, when open, `contents`.
    fn step(
        &self,
        step: TutorialStep,
        summary: &'static str,
        current: TutorialStep,
        skip: Option<fn(&mut App)>,
        contents: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        let TutorialStepInstructionsContent {
            open,
            icon,
            summary_text,
            skip_link: show_skip,
        } = tutorial_step_instructions(
            summary,
            &|s| current.completes(s),
            step,
            &|s| current == s,
            self.open,
            skip.is_some(),
        );
        let number = step.index().map_or(0, |ix| ix + 1);
        let circle = |bg: Hsla, border: Hsla| {
            div()
                .flex_none()
                .size(zpx(18.))
                .mr(SPACING())
                .rounded_full()
                .border_1()
                .border_color(border)
                .bg(bg)
                .flex()
                .items_center()
                .justify_center()
                .text_size(FONT_SIZE_SM())
        };
        let badge_text = if t.is_dark() {
            c(primer::GRAY_300)
        } else {
            gpui_kit::white()
        };
        let icon: AnyElement = match icon {
            TutorialStepIcon::GreenCheck => circle(c(primer::GREEN), c(primer::GREEN))
                .child(octicon(Octicon::Check, badge_text).size(zpx(12.)))
                .into_any_element(),
            TutorialStepIcon::Blue(number) => circle(c(primer::BLUE), c(primer::BLUE))
                .text_color(badge_text)
                .child(number)
                .into_any_element(),
            TutorialStepIcon::Empty(number) => circle(gpui_kit::transparent_black(), t.text)
                .text_color(t.text)
                .when(!open, |d| d.opacity(0.5))
                .child(number)
                .into_any_element(),
        };
        let entity = cx.entity().downgrade();
        let summary_row = div()
            .id(SharedString::from(format!("tutorial-step-{number}")))
            .a11y_row(summary_text.clone(), open)
            .aria_expanded(open)
            .flex()
            .flex_row()
            .items_center()
            .p(SPACING_DOUBLE())
            .when(open, |d| d.pb(SPACING()))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                entity
                    .update(cx, |this, cx| {
                        this.open = if this.open == Some(step) {
                            None
                        } else {
                            Some(step)
                        };
                        cx.notify();
                    })
                    .ok();
            })
            .child(icon)
            .child(
                div()
                    .text_color(if open { t.text } else { t.text_secondary })
                    .child(summary_text),
            )
            .child(
                div().ml_auto().child(match skip.filter(|_| show_skip) {
                    Some(skip) => link_button(
                        SharedString::from(format!("tutorial-skip-{number}")),
                        "Skip",
                        cx,
                    )
                    .font_weight(FontWeight::NORMAL)
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        skip(cx)
                    })
                    .into_any_element(),
                    None => octicon(
                        if open {
                            Octicon::ChevronUp
                        } else {
                            Octicon::ChevronDown
                        },
                        t.text,
                    )
                    .into_any_element(),
                }),
            );
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.box_border)
            .child(summary_row)
            .when(open, |d| {
                d.child(
                    // `details .contents`
                    div()
                        .pl(SPACING_TRIPLE())
                        .pb(SPACING_DOUBLE())
                        .pr(SPACING())
                        .text_size(FONT_SIZE())
                        .line_height(zpx(18.))
                        .child(contents),
                )
            })
            .into_any_element()
    }
}

/// `.description` + `.action` in a step.
fn contents(description: AnyElement, action: Option<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(div().mb(SPACING()).child(description))
        .children(action)
        .into_any_element()
}

/// `.action`: buttons then the shortcut.
fn action_row(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING())
        .children(children)
        .into_any_element()
}

impl Render for TutorialPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let (current, editor, readme) = {
            let s = self.state.read(cx);
            let readme = s.selected_repository().map(|r| r.path.join("README.md"));
            (s.selected_tutorial_step(), s.resolved_editor_name(), readme)
        };
        if self.synced != Some(current) {
            self.synced = Some(current);
            self.open = Some(current);
        }
        let pick_editor = if !current.completes(TutorialStep::PickEditor) {
            contents(
                paragraph(vec![
                    "It doesn’t look like you have a text editor installed. We can recommend "
                        .into(),
                    link_button("tutorial-vscode", SUGGESTED_EDITOR.0, cx)
                        .on_click(|_, _, cx| Dispatcher::open_url(SUGGESTED_EDITOR.1, cx))
                        .into_any_element()
                        .into(),
                    " or ".into(),
                    link_button("tutorial-atom", "Atom", cx)
                        .on_click(|_, _, cx| Dispatcher::open_url("https://atom.io", cx))
                        .into_any_element()
                        .into(),
                    ", but feel free to use any.".into(),
                ])
                .into_any_element(),
                Some(action_row(vec![
                    link_button("tutorial-have-editor", "I have an editor", cx)
                        .on_click(|_, _, cx| Dispatcher::skip_pick_editor_tutorial_step(cx))
                        .into_any_element(),
                ])),
            )
        } else {
            contents(
                paragraph(vec![
                    "Your default editor is ".into(),
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(editor.clone().unwrap_or_else(|| "not set".into()))
                        .into_any_element()
                        .into(),
                    ". You can change your preferred editor in ".into(),
                    link_button("tutorial-settings", mac_or("Settings", "options"), cx)
                        .on_click(|_, _, cx| {
                            Dispatcher::open_preferences(PreferencesTab::Integrations, cx)
                        })
                        .into_any_element()
                        .into(),
                ])
                .into_any_element(),
                None,
            )
        };
        let edit_file = contents(
            paragraph(vec![
                "Open this repository in your preferred text editor. Edit the ".into(),
                Inline::Element(code_ref("README.md", cx).into_any_element()),
                " file, save it, and come back.".into(),
            ])
            .into_any_element(),
            editor.as_ref().map(|_| {
                let readme = readme.clone().unwrap_or_else(PathBuf::new);
                action_row(vec![
                    button(
                        "tutorial-open-editor",
                        mac_or("Open Editor", "Open editor"),
                        cx,
                    )
                    .on_click(move |_, _, cx| Dispatcher::open_in_editor(readme.clone(), cx))
                    .into_any_element(),
                    kbd_group(&["⌘", "⇧", "A"], cx).into_any_element(),
                ])
            }),
        );
        let steps =
            vec![
            self.step(
                TutorialStep::PickEditor,
                "Install a text editor",
                current,
                Some(|cx| Dispatcher::skip_pick_editor_tutorial_step(cx)),
                pick_editor,
                cx,
            ),
            self.step(
                TutorialStep::CreateBranch,
                "Create a branch",
                current,
                None,
                contents(
                    div()
                        .child(format!(
                            "A branch allows you to work on different versions of a repository at \
                             one time. Create a branch by going into the branch menu in the top \
                             bar and clicking \"{}\".",
                            mac_or("New Branch", "New branch")
                        ))
                        .into_any_element(),
                    Some(action_row(vec![
                        kbd_group(&["⌘", "⇧", "N"], cx).into_any_element(),
                    ])),
                ),
                cx,
            ),
            self.step(
                TutorialStep::EditFile,
                "Edit a file",
                current,
                None,
                edit_file,
                cx,
            ),
            self.step(
                TutorialStep::MakeCommit,
                "Make a commit",
                current,
                None,
                contents(
                    div()
                        .child(
                            "A commit allows you to save sets of changes. In the “summary“ field \
                             in the bottom left, write a short message that describes the changes \
                             you made. When you’re done, click the blue Commit button to finish.",
                        )
                        .into_any_element(),
                    None,
                ),
                cx,
            ),
            self.step(
                TutorialStep::PushBranch,
                "Publish to GitHub",
                current,
                None,
                contents(
                    div()
                        .child(
                            "Publishing will “push”, or upload, your commits to this branch of \
                             your repository on GitHub. Publish using the third button in the top \
                             bar.",
                        )
                        .into_any_element(),
                    Some(action_row(vec![kbd_group(&["⌘", "P"], cx).into_any_element()])),
                ),
                cx,
            ),
            self.step(
                TutorialStep::OpenPullRequest,
                "Open a pull request",
                current,
                Some(|cx| Dispatcher::mark_pull_request_tutorial_step_complete(cx)),
                contents(
                    div()
                        .child(
                            "A pull request allows you to propose changes to the code. By opening \
                             one, you’re requesting that someone review and merge them. Since this \
                             is a demo repository, this pull request will be private.",
                        )
                        .into_any_element(),
                    Some(action_row(vec![
                        button("tutorial-open-pr", "", cx)
                            .gap(SPACING())
                            .role(Role::Link)
                            .aria_label(mac_or("Open Pull Request", "Open pull request"))
                            .child(mac_or("Open Pull Request", "Open pull request"))
                            .child(octicon(Octicon::LinkExternal, t.text_secondary).size(zpx(14.)))
                            .on_click(|_, _, cx| {
                                // `openPullRequest`: close the step first, then open
                                Dispatcher::mark_pull_request_tutorial_step_complete(cx);
                                let id = AppState::global(cx).read(cx).selected;
                                cx.spawn(async move |cx: &mut AsyncApp| {
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_millis(500))
                                        .await;
                                    if let Some(id) = id {
                                        cx.update(|cx| Dispatcher::create_pull_request(id, cx));
                                    }
                                })
                                .detach();
                            })
                            .into_any_element(),
                        kbd_group(&["⌘", "R"], cx).into_any_element(),
                    ])),
                ),
                cx,
            ),
        ];
        div()
            .id("tutorial-panel")
            .role(Role::Complementary)
            .aria_label("Tutorial")
            .w(TUTORIAL_PANEL_WIDTH())
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.text)
            .text_size(FONT_SIZE())
            .border_l_1()
            .border_color(t.box_border)
            .shadow(vec![BoxShadow {
                color: t.shadow,
                offset: point(zpx(0.), zpx(2.)),
                blur_radius: css_blur(7.),
                spread_radius: zpx(0.),
                inset: false,
            }])
            .overflow_y_scroll()
            .child(
                // `.titleArea`
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p(SPACING_TRIPLE())
                    .pl(SPACING_QUINT())
                    .pr(SPACING_DOUBLE())
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(
                        div()
                            .text_size(FONT_SIZE_LG())
                            .font_weight(FontWeight::LIGHT)
                            .child("Get started"),
                    )
                    .child(img("illustrations/required-status-check.svg").size(zpx(46.))),
            )
            .children(steps)
            .child(
                // `.footer`
                div()
                    .mt_auto()
                    .py(SPACING_DOUBLE())
                    .flex()
                    .justify_center()
                    .child(
                        button(
                            "tutorial-exit",
                            mac_or("Exit Tutorial", "Exit tutorial"),
                            cx,
                        )
                        .on_click(|_, _, cx| Dispatcher::exit_tutorial(cx)),
                    ),
            )
            // `.tutorial-panel-component { overflow-y: scroll }`
            .with_scrollbar()
            .overflow_scroll()
    }
}

/// `TutorialWelcome`: the Changes pane while the tutorial runs, drawn from
/// [`tutorial_welcome_content`].
pub fn tutorial_welcome(cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    // `103-product-name`
    let name = AppState::global(cx).read(cx).product_name().to_string();
    let TutorialWelcomeContent {
        title,
        text,
        definitions,
        image_alts,
    } = tutorial_welcome_content(&name);
    let definition = |definition: TutorialDefinition, alt: String| {
        div()
            .w(zpx(160.))
            .flex_none()
            .flex()
            .flex_col()
            .px(SPACING_HALF())
            .pb(SPACING_DOUBLE())
            .child(
                img(definition.image)
                    .role(Role::Image)
                    .aria_label(alt)
                    .size(zpx(48.))
                    .self_center(),
            )
            .child(
                paragraph(vec![
                    Inline::Element(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(definition.term)
                            .into_any_element(),
                    ),
                    definition.rest.into(),
                ])
                .mt(SPACING()),
            )
    };
    div()
        .id("tutorial-welcome")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .px(SPACING_TRIPLE())
        .overflow_y_scroll()
        .bg(t.background)
        .text_color(t.text)
        .text_size(FONT_SIZE())
        .child(
            // `.header`
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .my(SPACING_QUINT())
                .pt(SPACING_DOUBLE())
                .text_center()
                .child(
                    div()
                        .text_size(FONT_SIZE_XL())
                        .line_height(zpx(35.))
                        .font_weight(FontWeight::LIGHT)
                        .my(zpx(21.))
                        .child(title),
                )
                .child(div().my(SPACING_THIRD()).child(text)),
        )
        .child(
            // `.definitions`
            div()
                .max_w(zpx(600.))
                .flex()
                .flex_row()
                .flex_wrap()
                .justify_around()
                .children(
                    definitions
                        .into_iter()
                        .zip(image_alts)
                        .map(|(d, alt)| definition(d, alt)),
                ),
        )
}

/// `TutorialDone`: "You're done!" with three suggested actions, drawn from
/// [`tutorial_done_content`] for the selected (tutorial) repository (the
/// workspace marks the completion announced once it has been painted).
pub fn tutorial_done(cx: &App) -> impl IntoElement + use<> {
    let t = cx.ghd();
    let state = AppState::global(cx).read(cx);
    // `103-product-name`
    let name = state.product_name().to_string();
    let content = state
        .selected
        .and_then(|id| state.repository(id))
        .map(tutorial_done_content);
    let Some(TutorialDoneContent {
        heading,
        image_alt,
        actions,
    }) = content
    else {
        return div().id("tutorial-done");
    };
    let action = |icon: Octicon,
                  title: &'static str,
                  description: SharedString,
                  (label, on_click): (String, TutorialDoneAction),
                  cx: &App| {
        let id = match on_click {
            TutorialDoneAction::ShowGitHubExplore(_) => "tutorial-explore",
            TutorialDoneAction::ShowPopup(Popup::CreateRepository { .. }) => "tutorial-create",
            TutorialDoneAction::ShowPopup(_) => "tutorial-add",
        };
        // `SuggestedAction` with an `image`
        div()
            .flex()
            .flex_row()
            .items_center()
            .p(SPACING_DOUBLE())
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .bg(t.background)
            .child(
                div()
                    .flex_none()
                    .mr(SPACING_DOUBLE())
                    .child(octicon(icon, t.text).size(zpx(24.))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .mr(SPACING_DOUBLE())
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(zpx(18.))
                            .child(title),
                    )
                    .child(div().line_height(zpx(18.)).child(description)),
            )
            .child(
                button(id, label, cx).on_click(move |_, _, cx| match &on_click {
                    TutorialDoneAction::ShowGitHubExplore(repo) => {
                        Dispatcher::show_github_explore(*repo, cx)
                    }
                    TutorialDoneAction::ShowPopup(popup) => {
                        Dispatcher::show_popup(popup.clone(), cx)
                    }
                }),
            )
    };
    let mut actions = actions.into_iter();
    div()
        .id("tutorial-done")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .p(SPACING_QUAD())
        .overflow_y_scroll()
        .bg(t.background)
        .text_color(t.text)
        .text_size(FONT_SIZE())
        .child(
            div()
                .w_full()
                .max_w(zpx(600.))
                .flex()
                .flex_col()
                .child(
                    // `.header`
                    div()
                        .flex()
                        .flex_row()
                        .mb(SPACING_DOUBLE())
                        .child(
                            div()
                                .id("tutorial-done-header")
                                .a11y_live(format!(
                                    "You're done! You’ve learned the basics on how to use {name}."
                                ))
                                .flex_1()
                                .mr(SPACING_DOUBLE())
                                .child(
                                    div()
                                        .text_size(FONT_SIZE_XL())
                                        .line_height(zpx(35.))
                                        .font_weight(FontWeight::LIGHT)
                                        .child(heading),
                                )
                                .child(format!(
                                    "You’ve learned the basics on how to use {name}. Here are \
                                     some suggestions for what to do next."
                                )),
                        )
                        .children(image_alt.map(|alt| {
                            img("illustrations/admin-mentoring.svg")
                                .role(Role::Image)
                                .aria_label(alt)
                                .flex_none()
                                .self_end()
                                .w(zpx(73.))
                                .h(zpx(70.))
                        })),
                )
                .child(
                    // `SuggestedActionGroup`
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACING())
                        .children(actions.next().map(|a| {
                            action(
                                Octicon::Telescope,
                                "Explore projects on GitHub",
                                "Contribute to a project that interests you".into(),
                                a,
                                cx,
                            )
                        }))
                        .children(actions.next().map(|a| {
                            action(
                                Octicon::Plus,
                                "Create a new repository",
                                "Get started on a brand new project".into(),
                                a,
                                cx,
                            )
                        }))
                        .children(actions.next().map(|a| {
                            action(
                                Octicon::FileDirectory,
                                "Add a local repository",
                                format!("Work on an existing project in {name}").into(),
                                a,
                                cx,
                            )
                        })),
                ),
        )
}
