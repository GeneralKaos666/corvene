//! Remove Repositories: a Corvene extra with no GHD counterpart (flag
//! `269-bulk-remove-repositories`, see `.docs/deviations.md`). GHD removes
//! one repository at a time (`ConfirmRemoveRepository`,
//! `ui/remove-repository/confirm-remove-repository.tsx`); this dialog lists
//! every repository with a checkbox (narrowed by a filter box) and removes
//! the ticked ones from Corvene, with the same "Also move … to Trash" choice
//! (never for a missing repository, as GHD).

use std::collections::BTreeSet;

use corvene_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox, checkbox_tristate};

pub struct RemoveRepositoriesDialog {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    ticked: BTreeSet<u64>,
    move_to_trash: bool,
}

impl RemoveRepositoriesDialog {
    pub fn new(
        state: Entity<AppState>,
        ticked: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let handle = filter.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            state,
            filter,
            ticked: ticked.into_iter().collect(),
            move_to_trash: false,
        }
    }

    /// The repositories the filter shows, by name: (id, name, path, missing).
    fn shown(&self, cx: &App) -> Vec<(u64, String, String, bool)> {
        let query = self.filter.read(cx).value().trim().to_lowercase();
        self.state
            .read(cx)
            .sorted_repositories()
            .into_iter()
            .filter(|r| {
                query.is_empty()
                    || corvene_core::filter::fuzzy_match(&query, &r.name()).is_some()
                    || r.path.to_string_lossy().to_lowercase().contains(&query)
            })
            .map(|r| (r.id, r.name(), r.path.display().to_string(), r.missing))
            .collect()
    }

    fn toggle(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.ticked.remove(&id) {
            self.ticked.insert(id);
        }
        cx.notify();
    }
}

impl Render for RemoveRepositoriesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        // repositories removed elsewhere meanwhile drop out of the selection
        let existing: BTreeSet<u64> = self
            .state
            .read(cx)
            .repositories
            .iter()
            .map(|r| r.id)
            .collect();
        self.ticked.retain(|id| existing.contains(id));
        let shown = self.shown(cx);
        let shown_ids: Vec<u64> = shown.iter().map(|(id, ..)| *id).collect();
        let ticked_shown = shown_ids
            .iter()
            .filter(|id| self.ticked.contains(id))
            .count();
        let all_state = match ticked_shown {
            0 => Some(false),
            n if n == shown_ids.len() => Some(true),
            _ => None,
        };
        let picked: Vec<(u64, bool)> = {
            let s = self.state.read(cx);
            self.ticked
                .iter()
                .filter_map(|id| s.repository(*id).map(|r| (*id, r.missing)))
                .collect()
        };
        let any_on_disk = picked.iter().any(|(_, missing)| !missing);
        let trash = self.move_to_trash && any_on_disk;
        let weak = cx.weak_entity();
        let hover_bg = t.list_item_hover_background;

        let list = div()
            .id("remove-repositories-list")
            .h(zpx(300.))
            .overflow_y_scroll()
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_col()
            .when(shown.is_empty(), |d| {
                d.child(
                    div()
                        .p(SPACING())
                        .text_color(t.text_secondary)
                        .child("Sorry, I can't find that repository"),
                )
            })
            .children(shown.into_iter().map(|(id, name, path, missing)| {
                let weak = weak.clone();
                div()
                    .id(("remove-repositories-row", id))
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
                        weak.update(cx, |this, cx| this.toggle(id, cx)).ok();
                    })
                    .child(checkbox(
                        ("remove-repositories-check", id),
                        self.ticked.contains(&id),
                        false,
                        cx,
                    ))
                    .when(missing, |d| {
                        d.child(octicon(Octicon::Alert, t.text_secondary).flex_none())
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().truncate().child(name))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(FONT_SIZE_SM())
                                    .text_color(t.text_secondary)
                                    .child(path),
                            ),
                    )
            }))
            .with_scrollbar();

        let select_all = {
            let weak = weak.clone();
            let ids = shown_ids.clone();
            div()
                .id("remove-repositories-all")
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if all_state == Some(true) {
                            for id in &ids {
                                this.ticked.remove(id);
                            }
                        } else {
                            this.ticked.extend(ids.iter().copied());
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .child(checkbox_tristate(
                    "remove-repositories-all-check",
                    all_state,
                    shown_ids.is_empty(),
                    cx,
                ))
                .child("Select all")
        };

        let content = div()
            .w(crate::theme::fit_width(460.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "Choose the repositories to remove from Corvene. Their folders stay on disk \
                 unless you also move them to the Trash.",
            )
            .child(crate::widgets::filter_text_box(
                "remove-repositories-filter",
                &self.filter,
                Some(octicon(Octicon::Search, t.text_secondary)),
                window,
                cx,
            ))
            .child(select_all)
            .child(list)
            .when(any_on_disk, |d| {
                let weak = weak.clone();
                d.child(crate::widgets::checkbox_row(
                    "remove-repositories-trash",
                    trash,
                    // GHD `TrashNameLabel`
                    if cfg!(windows) {
                        "Also move these repositories to Recycle Bin"
                    } else {
                        "Also move these repositories to Trash"
                    },
                    move |value, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.move_to_trash = value;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            });

        let count = picked.len();
        let buttons = vec![
            // destructive, so Cancel is the default button (as GHD's
            // single-repository confirmation)
            DialogButton {
                id: "remove-repositories-cancel",
                label: "Cancel".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            },
            DialogButton {
                id: "remove-repositories-ok",
                label: match count {
                    1 => "Remove 1 Repository".into(),
                    n => format!("Remove {n} Repositories").into(),
                },
                primary: false,
                disabled: count == 0,
                on_click: Box::new(move |_, cx| {
                    if picked.is_empty() {
                        return;
                    }
                    Dispatcher::close_popup(cx);
                    for (id, missing) in &picked {
                        if trash && !missing {
                            Dispatcher::remove_repository_and_trash(*id, cx);
                        } else {
                            Dispatcher::remove_repository(*id, cx);
                        }
                    }
                }),
            },
        ];
        dialog(
            "dialog-remove-repositories",
            crate::context_menu::mac_or("Remove Repositories", "Remove repositories"),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
