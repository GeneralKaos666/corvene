//! Corvene addition (`785-embedded-repo-commit`, no GHD counterpart:
//! GHD's commit skips an untracked folder that is a git repository): add
//! nested repositories as submodules (with an `origin`) or as pointers to
//! their commit, then commit (from the commit form) or not (from the changes
//! list's menu). Laid out like `unknown_authors.rs`, the "do not show again"
//! box like `discard_changes.rs`.

use corvene_core::Dispatcher;
use corvene_core::commit_checks::CommitChecks;
use corvene_git::EmbeddedRepository;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog_with_kind};
use crate::theme::ActiveGhdTheme;
use crate::theme::mono_font;
use crate::theme::sizes::*;

pub struct AddEmbeddedRepositoriesDialog {
    repo: u64,
    repositories: Vec<EmbeddedRepository>,
    commit: Option<(String, String, CommitChecks)>,
    dont_show_again: bool,
}

impl AddEmbeddedRepositoriesDialog {
    pub fn new(
        repo: u64,
        repositories: Vec<EmbeddedRepository>,
        commit: Option<(String, String, CommitChecks)>,
    ) -> Self {
        Self {
            repo,
            repositories,
            commit,
            dont_show_again: false,
        }
    }
}

impl Render for AddEmbeddedRepositoriesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let any_url = self.repositories.iter().any(|r| r.url.is_some());
        let only_pointers = !any_url;
        let lead = match (self.repositories.as_slice(), any_url) {
            ([one], true) => format!(
                "{} is a Git repository. Add it to this repository as a submodule?",
                one.path
            ),
            ([one], false) => format!(
                "{} is a Git repository. Add it to this repository as a pointer to its current \
                 commit?",
                one.path
            ),
            (_, true) => "These folders are Git repositories. Add them to this repository as \
                          submodules?"
                .to_string(),
            (_, false) => "These folders are Git repositories. Add them to this repository as \
                           pointers to their current commits?"
                .to_string(),
        };
        let weak = cx.weak_entity();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(lead)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .pl(SPACING_DOUBLE())
                    .font_family(mono_font())
                    .children(self.repositories.iter().map(|r| {
                        div()
                            .flex()
                            .flex_row()
                            .gap(SPACING())
                            .child(format!("• {}", r.path))
                            .child(div().text_color(t.text_secondary).child(match &r.url {
                                Some(url) => url.clone(),
                                None => "no origin remote".to_string(),
                            }))
                    })),
            )
            .when(self.repositories.iter().any(|r| r.url.is_none()), |d| {
                d.child(
                    "A repository without an origin remote is added as a pointer to its \
                     current commit only, without a .gitmodules entry: clones of this \
                     repository will not know where to get it.",
                )
            })
            // the note can be dismissed when it is all the dialog has to say
            .when(only_pointers && self.commit.is_some(), |d| {
                d.child(crate::widgets::checkbox_row(
                    "embedded-repositories-dont-show",
                    self.dont_show_again,
                    "Do not show this message again",
                    move |value, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.dont_show_again = value;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            });
        let ok_label: SharedString = match (&self.commit, any_url) {
            (Some(_), _) => mac_or("Add and Commit", "Add and commit").into(),
            (None, true) if self.repositories.len() == 1 => {
                mac_or("Add as Submodule", "Add as submodule").into()
            }
            (None, true) => mac_or("Add as Submodules", "Add as submodules").into(),
            (None, false) => "Add".into(),
        };
        let (repo, repositories, commit, dont_show) = (
            self.repo,
            self.repositories.clone(),
            self.commit.clone(),
            self.dont_show_again,
        );
        dialog_with_kind(
            "add-embedded-repositories",
            DialogKind::Normal,
            if self.repositories.len() == 1 {
                mac_or("Nested Git Repository", "Nested Git repository")
            } else {
                mac_or("Nested Git Repositories", "Nested Git repositories")
            },
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "add-embedded-repositories-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "add-embedded-repositories-ok",
                    label: ok_label,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if dont_show {
                            Dispatcher::update_settings(cx, |s| {
                                s.hide_embedded_repository_note = true
                            });
                        }
                        match &commit {
                            Some((summary, description, checks)) => Dispatcher::commit_with(
                                repo,
                                summary.clone(),
                                description.clone(),
                                CommitChecks {
                                    allow_oversized: true,
                                    embedded: Some(repositories.clone()),
                                    ..checks.clone()
                                },
                                cx,
                            ),
                            None => Dispatcher::add_embedded_repositories(
                                repo,
                                repositories.clone(),
                                cx,
                            ),
                        }
                    }),
                },
            }
            .into_buttons(),
            close,
            window,
            cx,
        )
    }
}
