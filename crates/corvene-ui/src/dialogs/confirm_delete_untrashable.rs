//! `PopupType.DiscardChangesRetry` - GHD
//! `ui/discard-changes/discard-changes-retry-dialog.tsx`: Discard Changes
//! could not move files to the Trash; "Permanently Discard Changes"
//! discards them without it, and "Do not show this message again" clears
//! `askForConfirmationOnDiscardChangesPermanently` (Settings › Prompts).
//!
//! Differences: GHD stops the whole discard at the first file the Trash
//! refuses and retries all of it permanently; Corvene discards everything
//! else and keeps only the refused files' changes, which this dialog lists
//! (GHD names none) before discarding them for good.

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::scrollbar::ScrollbarExt;
use crate::theme::mono_font;
use crate::theme::sizes::*;
use crate::widgets::checkbox_row;

pub struct ConfirmDeleteUntrashableDialog {
    repo: u64,
    paths: Vec<String>,
    dont_show_again: bool,
}

impl ConfirmDeleteUntrashableDialog {
    pub fn new(repo: u64, paths: Vec<String>) -> Self {
        Self {
            repo,
            paths,
            dont_show_again: false,
        }
    }
}

impl Render for ConfirmDeleteUntrashableDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        // GHD `TrashNameLabel`
        let trash = if cfg!(windows) {
            "Recycle Bin"
        } else {
            "Trash"
        };
        let weak = cx.weak_entity();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!("Failed to discard changes to {trash}."))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child("Common reasons are:")
                    .child(format!(
                        "\u{2022} The {trash} is configured to delete items immediately."
                    ))
                    .child("\u{2022} Restricted access to move the file(s)."),
            )
            .child(
                div()
                    .id("untrashable-file-list")
                    .max_h(zpx(175.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(self.paths.iter().map(|p| {
                        div()
                            .line_height(zpx(18.))
                            .font_family(mono_font())
                            .child(crate::format::display_path(p))
                    }))
                    .with_scrollbar(),
            )
            .child(format!(
                "These changes will be unrecoverable from the {trash}."
            ))
            .child(checkbox_row(
                "untrashable-dont-show",
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
            ));
        let repo = self.repo;
        let paths = self.paths.clone();
        let dont_show_again = self.dont_show_again;
        dialog_with_kind(
            "dialog-confirm-delete-untrashable",
            DialogKind::Error,
            mac_or(
                "Discarded Changes Will Be Unrecoverable",
                "Discarded changes will be unrecoverable",
            ),
            content,
            vec![
                DialogButton {
                    id: "untrashable-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "untrashable-delete",
                    label: mac_or("Permanently Discard Changes", "Permanently discard changes")
                        .into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| {
                                s.confirm_discard_changes_permanently = false
                            });
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::delete_untrashable(repo, paths.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
