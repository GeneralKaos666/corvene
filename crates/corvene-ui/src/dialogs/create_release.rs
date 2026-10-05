//! Corvene `346-releases`: Create Release…, a Corvene extra with no GitHub
//! Desktop counterpart (releases are made on github.com). The tag (one of
//! the repository's, or New tag… with a name, made at the commit the dialog
//! was opened on or the current branch's tip), a title, the notes (typed,
//! or Generate release notes: GitHub's, or the local history's on a host
//! that cannot generate them, `Dispatcher::generate_release_notes`), and
//! the pre-release and draft switches. Create Release / Save Draft hands it
//! to `Dispatcher::create_release`.

use corvene_core::releases::ReleaseDraft;
use corvene_core::{AppState, Dispatcher};
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon, spin};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    SelectHandler, SelectItem, checkbox_row, labeled, resizable_text_area, select_button_items,
    small_button, text_box,
};

pub struct CreateReleaseDialog {
    state: Entity<AppState>,
    repo: u64,
    /// The picked existing tag; `None` is New tag….
    tag: Option<String>,
    new_tag: Entity<InputState>,
    /// The commit a new tag is made at.
    sha: Option<String>,
    title: Entity<InputState>,
    body: Entity<TextareaState>,
    prerelease: bool,
    draft: bool,
    /// Generate release notes is running.
    generating: bool,
    /// The last generation's outcome: an error, or whether the notes came
    /// from the local history.
    generated: Option<Result<bool, String>>,
}

impl CreateReleaseDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        tag: Option<String>,
        sha: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let new_tag = cx.new(|cx| InputState::new(window, cx).placeholder("v1.0.0"));
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Release title"));
        let body = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Describe this release")
                .rows(10)
        });
        for input in [&new_tag, &title] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&body, |_, _, cx| cx.notify()).detach();
        cx.observe_in(&state, window, |this, state, window, cx| {
            // the generated notes arrive through the state
            let waiting = this.generating;
            let done = state
                .read(cx)
                .repo_states
                .get(&this.repo)
                .is_some_and(|rs| rs.generated_release_notes.is_some());
            if waiting && done {
                this.take_generated(window, cx);
            }
            cx.notify();
        })
        .detach();
        if tag.is_none() {
            let handle = new_tag.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        } else {
            let handle = title.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
        Self {
            state,
            repo,
            tag,
            new_tag,
            sha,
            title,
            body,
            prerelease: false,
            draft: false,
            generating: false,
            generated: None,
        }
    }

    /// The tag name the release is for, and whether it is new.
    fn tag_name(&self, cx: &App) -> (String, bool) {
        match &self.tag {
            Some(tag) => (tag.clone(), false),
            None => (
                crate::dialogs::branch_dialogs::sanitize_ref_name(&self.new_tag.read(cx).value()),
                true,
            ),
        }
    }

    fn take_generated(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(result) = Dispatcher::take_generated_release_notes(self.repo, cx) else {
            return;
        };
        self.generating = false;
        match result {
            Ok(notes) => {
                let title_empty = self.title.read(cx).value().trim().is_empty();
                self.body
                    .update(cx, |s, cx| s.set_value(notes.body, window, cx));
                if title_empty && !notes.title.trim().is_empty() {
                    self.title
                        .update(cx, |s, cx| s.set_value(notes.title, window, cx));
                }
                self.generated = Some(Ok(notes.local));
            }
            Err(err) => self.generated = Some(Err(err)),
        }
        cx.notify();
    }

    fn generate(&mut self, cx: &mut Context<Self>) {
        let (tag, new_tag) = self.tag_name(cx);
        if tag.is_empty() || self.generating {
            return;
        }
        self.generating = true;
        self.generated = None;
        cx.notify();
        let target = if new_tag { self.sha.clone() } else { None };
        Dispatcher::generate_release_notes(self.repo, tag, target, cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let (tag, new_tag) = self.tag_name(cx);
        if tag.is_empty() {
            return;
        }
        let draft = ReleaseDraft {
            tag,
            new_tag,
            target_sha: self.sha.clone(),
            title: self.title.read(cx).value().trim().to_string(),
            body: self.body.read(cx).value().trim_end().to_string(),
            prerelease: self.prerelease,
            draft: self.draft,
        };
        Dispatcher::create_release(self.repo, draft, cx);
    }
}

impl Render for CreateReleaseDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (tags, current_branch, read_only) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let tags: Vec<String> = rs
                .and_then(|r| r.branch_list_tags.clone())
                .map(|tags| tags.iter().map(|(name, _)| name.clone()).collect())
                .unwrap_or_default();
            let branch = rs
                .and_then(|r| r.info.as_ref())
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone());
            let read_only = s
                .repository(self.repo)
                .and_then(|r| r.non_fork_github())
                .is_some_and(|gh| !gh.has_write_permission());
            (tags, branch, read_only)
        };
        // the tag the menu was opened on may not be listed yet
        let mut tag_options: Vec<String> = tags;
        if let Some(tag) = &self.tag
            && !tag_options.contains(tag)
        {
            tag_options.insert(0, tag.clone());
        }
        let (tag, new_tag) = self.tag_name(cx);
        let mut items: Vec<SelectItem> = vec![SelectItem::Option("New tag…".into())];
        if !tag_options.is_empty() {
            items.push(SelectItem::Separator);
            items.extend(
                tag_options
                    .iter()
                    .map(|t| SelectItem::Option(SharedString::from(t.clone()))),
            );
        }
        let selected = match &self.tag {
            None => Some(0),
            Some(tag) => tag_options.iter().position(|t| t == tag).map(|ix| ix + 1),
        };
        let this = cx.weak_entity();
        let options = tag_options.clone();
        let on_select: SelectHandler = std::rc::Rc::new({
            let this = this.clone();
            move |ix, window, cx| {
                let picked = if ix == 0 {
                    None
                } else {
                    options.get(ix - 1).cloned()
                };
                this.update(cx, |d, cx| {
                    d.tag = picked;
                    d.generated = None;
                    if d.tag.is_none() {
                        let handle = d.new_tag.read(cx).focus_handle(cx);
                        window.focus(&handle, cx);
                    }
                    cx.notify();
                })
                .ok();
            }
        });
        let picker_value: SharedString = match &self.tag {
            Some(tag) => tag.clone().into(),
            None => "New tag…".into(),
        };
        let target_text = match (&self.sha, &current_branch) {
            (Some(sha), _) if new_tag => format!("at {}", &sha[..sha.len().min(7)]),
            (None, Some(branch)) if new_tag => format!("at the tip of {branch}"),
            _ => String::new(),
        };
        let tag_row = div()
            .flex()
            .flex_row()
            .items_end()
            .gap(SPACING_HALF())
            .child(labeled(
                "Tag",
                select_button_items(
                    "release-tag",
                    picker_value,
                    items,
                    selected,
                    false,
                    on_select,
                    cx,
                ),
                cx,
            ))
            .when(new_tag, |d| {
                d.child(labeled(
                    "New tag name",
                    text_box("release-new-tag", &self.new_tag, None, window, cx),
                    cx,
                ))
            });
        let generate_disabled = tag.is_empty() || self.generating;
        let generate = {
            let this = this.clone();
            small_button(
                "release-generate-notes",
                mac_or("Generate Release Notes", "Generate release notes"),
                cx,
            )
            .when(generate_disabled, |d| d.opacity(0.6))
            .on_click(move |_, _, cx| {
                if !generate_disabled {
                    this.update(cx, |d, cx| d.generate(cx)).ok();
                }
            })
        };
        let notes_note: Option<AnyElement> = if self.generating {
            Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(spin(
                        octicon(Octicon::SyncClockwise, t.text_secondary),
                        "release-generate-spin",
                    ))
                    .child("Generating…")
                    .into_any_element(),
            )
        } else {
            match &self.generated {
                Some(Ok(true)) => Some(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child("Generated from the local history (GitHub could not generate notes here).")
                        .into_any_element(),
                ),
                Some(Ok(false)) => Some(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child("Generated by GitHub.")
                        .into_any_element(),
                ),
                Some(Err(err)) => Some(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.form_error_text)
                        .child(format!("Could not generate notes: {err}"))
                        .into_any_element(),
                ),
                None => None,
            }
        };
        let this_pre = this.clone();
        let this_draft = this.clone();
        let content = div()
            .w(crate::theme::fit_width(560.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(tag_row)
            .when(!target_text.is_empty(), |d| {
                d.child(
                    div()
                        .mt(-SPACING_HALF())
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(format!("The tag is created on GitHub {target_text}.")),
                )
            })
            .child(labeled(
                "Title",
                text_box("release-title", &self.title, None, window, cx),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_THIRD())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(div().flex_1().child("Notes"))
                            .child(generate),
                    )
                    .child(
                        div()
                            .border_1()
                            .border_color(t.box_border_contrast)
                            .rounded(BORDER_RADIUS())
                            .bg(t.box_background)
                            .overflow_hidden()
                            .child(resizable_text_area(
                                "release-body",
                                Textarea::new(&self.body),
                                cx,
                            )),
                    )
                    .children(notes_note),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_THIRD())
                    .child(checkbox_row(
                        "release-prerelease",
                        self.prerelease,
                        "Set as a pre-release",
                        move |on, _, cx| {
                            this_pre
                                .update(cx, |d, cx| {
                                    d.prerelease = on;
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    ))
                    .child(checkbox_row(
                        "release-draft",
                        self.draft,
                        "Save as a draft (not published yet)",
                        move |on, _, cx| {
                            this_draft
                                .update(cx, |d, cx| {
                                    d.draft = on;
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    )),
            )
            .when(read_only, |d| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.dialog_warning)
                        .child("Your account can only read this repository, so GitHub will refuse the release."),
                )
            });
        let disabled = tag.is_empty();
        let ok_label = if self.draft {
            mac_or("Save Draft", "Save draft")
        } else {
            mac_or("Create Release", "Create release")
        };
        dialog(
            "dialog-create-release",
            mac_or("Create a Release", "Create a release"),
            content,
            vec![
                DialogButton {
                    id: "release-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "release-create",
                    label: ok_label.into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if !disabled {
                            this.update(cx, |d, cx| d.submit(cx)).ok();
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
