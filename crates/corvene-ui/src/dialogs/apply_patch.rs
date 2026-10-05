//! Apply Patch: a Corvene extra with no GHD counterpart (flag
//! `1106-apply-patch`, see `.docs/deviations.md`). Shows the files a patch
//! touches (`corvene_git::PatchPreview`) and whether it applies as it is;
//! a mailbox from `git format-patch` lists its commits and can be applied
//! as commits (`git am`) or as uncommitted changes.

use std::sync::Arc;

use corvene_core::Dispatcher;
use corvene_git::{PatchFileChange, PatchPreview};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{IS_MAC, mac_or};
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font};
use crate::widgets::checkbox_row;

pub struct ApplyPatchDialog {
    repo: u64,
    name: String,
    patch: Arc<Vec<u8>>,
    preview: PatchPreview,
    /// A mailbox's patches become commits (`git am`).
    as_commits: bool,
}

impl ApplyPatchDialog {
    pub fn new(repo: u64, name: String, patch: Arc<Vec<u8>>, preview: PatchPreview) -> Self {
        let as_commits = preview.is_mailbox();
        Self {
            repo,
            name,
            patch,
            preview,
            as_commits,
        }
    }
}

impl Render for ApplyPatchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let preview = &self.preview;
        let mailbox = preview.is_mailbox();
        let as_commits = self.as_commits && mailbox;
        let weak = cx.weak_entity();

        let status = if preview.applies_cleanly {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .child(octicon(Octicon::CheckCircleFill, t.color_new).flex_none())
                .child("The patch applies cleanly.")
        } else {
            let reason = preview
                .check_error
                .as_deref()
                .and_then(|e| e.lines().find(|l| !l.trim().is_empty()))
                .map(|l| l.trim().to_string());
            div()
                .flex()
                .flex_row()
                .gap(SPACING_HALF())
                .child(octicon(Octicon::Alert, t.text_secondary).flex_none())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            "The patch does not apply cleanly. Corvene will try a three-way \
                             merge, which can leave conflicts for you to resolve.",
                        )
                        .when_some(reason, |d, reason| {
                            d.child(
                                div()
                                    .text_size(FONT_SIZE_SM())
                                    .text_color(t.text_secondary)
                                    .font_family(mono_font())
                                    .truncate()
                                    .child(reason),
                            )
                        }),
                )
        };

        let file_rows = preview.files.iter().enumerate().map(|(ix, file)| {
            let (icon, color) = match file.change {
                PatchFileChange::Added => (Octicon::DiffAdded, t.color_new),
                PatchFileChange::Deleted => (Octicon::DiffRemoved, t.color_deleted),
                PatchFileChange::Renamed => (Octicon::DiffRenamed, t.color_renamed),
                PatchFileChange::Modified => (Octicon::DiffModified, t.color_modified),
            };
            let label = match &file.old_path {
                Some(old) => format!("{old} → {}", file.path),
                None => file.path.clone(),
            };
            let counts = match (file.added, file.deleted) {
                (Some(added), Some(deleted)) => div()
                    .flex()
                    .flex_row()
                    .gap(SPACING_HALF())
                    .flex_none()
                    .font_family(mono_font())
                    .text_size(FONT_SIZE_SM())
                    .child(div().text_color(t.color_new).child(format!("+{added}")))
                    .child(
                        div()
                            .text_color(t.color_deleted)
                            .child(format!("−{deleted}")),
                    ),
                _ => div()
                    .flex_none()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child("binary"),
            };
            div()
                .id(("apply-patch-file", ix))
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .px(SPACING())
                .py(SPACING_HALF())
                .child(octicon(icon, color).flex_none())
                .child(div().flex_1().min_w_0().truncate().child(label))
                .child(counts)
        });
        let files = div()
            .id("apply-patch-files")
            .max_h(zpx(220.))
            .overflow_y_scroll()
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_col()
            .children(file_rows)
            .with_scrollbar();

        let commits = mailbox.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(SPACING_HALF())
                .children(preview.commits.iter().map(|subject| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .child(octicon(Octicon::GitCommit, t.text_secondary).flex_none())
                        .child(div().min_w_0().truncate().child(subject.clone()))
                }))
                .child(checkbox_row(
                    "apply-patch-commits",
                    as_commits,
                    "Create a commit for each patch (git am)",
                    move |checked, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.as_commits = checked;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
        });

        let file_count = preview.files.len();
        let content = div()
            .w(crate::theme::fit_width(520.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(SPACING_HALF())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.name.clone()),
                    )
                    .child(div().text_color(t.text_secondary).child(match file_count {
                        1 => "changes 1 file".to_string(),
                        n => format!("changes {n} files"),
                    })),
            )
            .child(status)
            .children(commits)
            .child(files);

        let commit_count = preview.commits.len();
        let (repo, patch) = (self.repo, self.patch.clone());
        let label: SharedString = match (as_commits, commit_count) {
            (true, 1) => mac_or("Apply as Commit", "Apply as commit").into(),
            (true, n) if IS_MAC => format!("Apply as {n} Commits").into(),
            (true, n) => format!("Apply as {n} commits").into(),
            (false, _) => mac_or("Apply Patch", "Apply patch").into(),
        };
        let buttons = vec![
            DialogButton {
                id: "apply-patch-cancel",
                label: "Cancel".into(),
                primary: false,
                disabled: false,
                on_click: Box::new(close),
            },
            DialogButton {
                id: "apply-patch-ok",
                label,
                primary: true,
                disabled: false,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::apply_patch(
                        repo,
                        patch.clone(),
                        file_count,
                        commit_count,
                        as_commits,
                        cx,
                    )
                }),
            },
        ];
        dialog(
            "apply-patch",
            mac_or("Apply Patch", "Apply patch"),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
