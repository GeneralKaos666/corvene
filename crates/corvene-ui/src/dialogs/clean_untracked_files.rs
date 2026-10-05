//! Clean Untracked Files: a Corvene extra with no GHD counterpart (flag
//! `1105-clean-untracked-files`, see `.docs/deviations.md`). The `git clean
//! -n -d` dry run (`RepositoryState::clean_preview`) with a checkbox per
//! path, all ticked; Include ignored files reruns it with `-x`. Built like
//! Remove Repositories (`dialogs::remove_repositories`).

use std::collections::BTreeSet;

use corvene_core::{AppState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox, checkbox_row, checkbox_tristate};

pub struct CleanUntrackedFilesDialog {
    state: Entity<AppState>,
    repo: u64,
    /// Paths the user unticked (new dry runs keep them unticked).
    unticked: BTreeSet<String>,
}

impl CleanUntrackedFilesDialog {
    pub fn new(state: Entity<AppState>, repo: u64, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            unticked: BTreeSet::new(),
        }
    }

    fn preview(&self, cx: &App) -> corvene_core::clean_untracked::CleanPreview {
        self.state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|rs| rs.clean_preview.clone())
            .unwrap_or_else(|| corvene_core::clean_untracked::CleanPreview {
                loading: true,
                ..Default::default()
            })
    }

    fn toggle(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.unticked.remove(path) {
            self.unticked.insert(path.to_string());
        }
        cx.notify();
    }
}

impl Render for CleanUntrackedFilesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let repo = self.repo;
        let close =
            move |_: &mut Window, cx: &mut App| Dispatcher::close_clean_untracked_files(repo, cx);
        let preview = self.preview(cx);
        let include_ignored = preview.include_ignored;
        let ticked: Vec<String> = preview
            .paths
            .iter()
            .filter(|p| !self.unticked.contains(*p))
            .cloned()
            .collect();
        let all_state = match ticked.len() {
            0 => Some(false),
            n if n == preview.paths.len() => Some(true),
            _ => None,
        };
        let weak = cx.weak_entity();
        let hover_bg = t.list_item_hover_background;

        let message: Option<String> = if let Some(error) = &preview.error {
            Some(error.clone())
        } else if preview.paths.is_empty() && !preview.loading {
            Some(if include_ignored {
                "There are no untracked or ignored files.".into()
            } else {
                "There are no untracked files.".into()
            })
        } else if preview.paths.is_empty() {
            Some("Looking for files…".into())
        } else {
            None
        };
        let list = div()
            .id("clean-untracked-list")
            .h(zpx(260.))
            .overflow_y_scroll()
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_col()
            .when_some(message, |d, message| {
                d.child(
                    div()
                        .p(SPACING())
                        .text_color(t.text_secondary)
                        .child(message),
                )
            })
            .children(preview.paths.iter().enumerate().map(|(ix, path)| {
                let weak = weak.clone();
                let folder = path.ends_with('/');
                let toggled = path.clone();
                div()
                    .id(("clean-untracked-row", ix))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .px(SPACING())
                    .py(SPACING_HALF())
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(move |_, _, cx| {
                        weak.update(cx, |this, cx| this.toggle(&toggled, cx)).ok();
                    })
                    .child(checkbox(
                        ("clean-untracked-check", ix),
                        !self.unticked.contains(path),
                        false,
                        cx,
                    ))
                    .child(
                        octicon(
                            if folder {
                                Octicon::FileDirectory
                            } else {
                                Octicon::File
                            },
                            t.text_secondary,
                        )
                        .flex_none(),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(path.clone()))
            }))
            .with_scrollbar();

        let select_all = {
            let weak = weak.clone();
            let paths = preview.paths.clone();
            div()
                .id("clean-untracked-all")
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if all_state == Some(true) {
                            this.unticked.extend(paths.iter().cloned());
                        } else {
                            this.unticked.clear();
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .child(checkbox_tristate(
                    "clean-untracked-all-check",
                    all_state,
                    preview.paths.is_empty(),
                    cx,
                ))
                .child("Select all")
        };

        let content = div()
            .w(crate::theme::fit_width(480.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "These files and folders are not tracked by Git. The ticked ones will be \
                 deleted permanently; they do not go to the Trash.",
            )
            .child(select_all)
            .child(list)
            .child(checkbox_row(
                "clean-untracked-ignored",
                include_ignored,
                "Include ignored files",
                move |value, _, cx| Dispatcher::load_clean_preview(repo, value, cx),
                cx,
            ))
            .when(include_ignored, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(SPACING_HALF())
                        .text_color(t.text_secondary)
                        .child(octicon(Octicon::Alert, t.text_secondary).flex_none())
                        .child(div().flex_1().min_w_0().child(
                            "Ignored files are often build output or local settings, such as \
                             .env files, that exist nowhere else.",
                        )),
                )
            });

        let count = ticked.len();
        let buttons = vec![
            // destructive, so Cancel is the default button
            DialogButton {
                id: "clean-untracked-cancel",
                label: "Cancel".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            },
            DialogButton {
                id: "clean-untracked-ok",
                label: match count {
                    1 => mac_or("Delete 1 Item", "Delete 1 item").into(),
                    n if crate::context_menu::IS_MAC => format!("Delete {n} Items").into(),
                    n => format!("Delete {n} items").into(),
                },
                primary: false,
                disabled: count == 0 || preview.loading,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::clean_untracked_files(repo, ticked.clone(), include_ignored, cx)
                }),
            },
        ];
        dialog(
            "clean-untracked-files",
            mac_or("Clean Untracked Files", "Clean untracked files"),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
