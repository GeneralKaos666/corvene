//! Corvene `1221-stacked-branch-refs`: a squash, reorder or message edit
//! replays commits other local branches point at. Styled like GHD's
//! `multi-commit-operation/dialog/warn-force-push-dialog.tsx`; GHD itself
//! rewrites the commits without a word and leaves those branches on the old
//! ones (`lib/git/squash.ts`, `lib/git/reorder.ts`).

use corvene_core::Dispatcher;
use corvene_core::stacked_refs::StackedOp;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog_with_kind};
use crate::theme::sizes::*;
use crate::widgets::{Inline, checkbox, code_ref, paragraph};

/// How many branch names the text lists before "and N more".
const LISTED: usize = 5;

/// `Popup::WarnStackedBranches`
pub struct WarnStackedBranchesDialog {
    repo: u64,
    branches: Vec<String>,
    update_refs: corvene_git::UpdateRefs,
    op: StackedOp,
    /// "Move these branches too", ticked by default.
    move_branches: bool,
}

impl WarnStackedBranchesDialog {
    pub fn new(
        repo: u64,
        branches: Vec<String>,
        update_refs: corvene_git::UpdateRefs,
        op: StackedOp,
    ) -> Self {
        Self {
            repo,
            branches,
            update_refs,
            op,
            move_branches: true,
        }
    }
}

/// What the operation does, starting the sentence, and its button.
fn doing(op: &StackedOp) -> (&'static str, &'static str) {
    match op {
        StackedOp::Squash { .. } | StackedOp::Autosquash { .. } => {
            ("Squashing", mac_or("Begin Squash", "Begin squash"))
        }
        StackedOp::Reorder { .. } => ("Reordering", mac_or("Begin Reorder", "Begin reorder")),
        StackedOp::Reword { .. } => (
            "Editing the message",
            mac_or("Edit Message", "Edit message"),
        ),
    }
}

impl Render for WarnStackedBranchesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, move_branches) = (self.repo, self.move_branches);
        let (op, update_refs) = (self.op.clone(), self.update_refs.clone());
        let count = self.branches.len();
        let (doing, begin) = doing(&self.op);
        let squashing = matches!(
            self.op,
            StackedOp::Squash { .. } | StackedOp::Autosquash { .. }
        );
        let mut parts: Vec<Inline> = vec![
            format!(
                "{doing} rewrites {} at: ",
                if count == 1 {
                    "a commit another branch points"
                } else {
                    "commits other branches point"
                }
            )
            .into(),
        ];
        for (i, name) in self.branches.iter().take(LISTED).enumerate() {
            if i > 0 {
                parts.push(", ".into());
            }
            parts.push(code_ref(name.clone(), cx).into_any_element().into());
        }
        if count > LISTED {
            parts.push(format!(" and {} more", count - LISTED).into());
        }
        parts.push(".".into());
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(parts))
            .child(match (count == 1, squashing) {
                (true, true) => "Moved along, it points at the rewritten commit (the combined one if its commit is squashed). Otherwise it stays on the old commit.",
                (true, false) => "Moved along, it points at the rewritten commit. Otherwise it stays on the old commit.",
                (false, true) => "Moved along, each points at its rewritten commit (the combined one if its commit is squashed). Otherwise they stay on the old commits.",
                (false, false) => "Moved along, each points at its rewritten commit. Otherwise they stay on the old commits.",
            })
            .child(
                div()
                    .id("stacked-branches-move")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.move_branches = !this.move_branches;
                        cx.notify();
                    }))
                    .child(checkbox("stacked-branches-move-box", move_branches, false, cx))
                    .child(if count == 1 {
                        "Move this branch too"
                    } else {
                        "Move these branches too"
                    }),
            );
        dialog_with_kind(
            "dialog-warn-stacked-branches",
            DialogKind::Warning,
            mac_or(
                "Other Branches Use These Commits",
                "Other branches use these commits",
            ),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "stacked-branches-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "stacked-branches-ok",
                    label: begin.into(),
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::answer_stacked_branches(
                            repo,
                            op.clone(),
                            update_refs.clone(),
                            move_branches,
                            cx,
                        )
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
