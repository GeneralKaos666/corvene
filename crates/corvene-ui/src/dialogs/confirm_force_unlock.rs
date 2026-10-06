//! Force Unlock: a Corvene extra with no GHD counterpart (flag
//! `1113-lfs-locks`, see `.docs/deviations.md`). A repository admin
//! releasing a Git LFS lock someone else holds (`git lfs unlock --force`)
//! confirms first, as the holder may still be editing the file. Built like
//! Discard Stash (`dialogs::history_dialogs`).

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog_with_kind};
use crate::theme::sizes::*;

pub struct ConfirmForceUnlockDialog {
    repo: u64,
    path: String,
    owner: String,
}

impl ConfirmForceUnlockDialog {
    pub fn new(repo: u64, path: String, owner: String) -> Self {
        Self { repo, path, owner }
    }
}

impl Render for ConfirmForceUnlockDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, path) = (self.repo, self.path.clone());
        let owner = if self.owner.is_empty() {
            "someone else".to_string()
        } else {
            self.owner.clone()
        };
        let content = div()
            .w(crate::theme::fit_width(400.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                div()
                    .child(format!("{} is locked by {owner}.", self.path))
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(
                "Unlocking it lets others change it while they may still be working on it. \
                 Their changes are not lost, but merging them may be hard.",
            );
        dialog_with_kind(
            "dialog-force-unlock",
            DialogKind::Warning,
            mac_or("Force Unlock File?", "Force unlock file?"),
            content,
            OkCancelButtonGroup {
                destructive: true,
                cancel: GroupButtonSpec {
                    id: "force-unlock-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "force-unlock-ok",
                    label: mac_or("Force Unlock", "Force unlock").into(),
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::unlock_lfs_files(repo, vec![path.clone()], true, cx);
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
