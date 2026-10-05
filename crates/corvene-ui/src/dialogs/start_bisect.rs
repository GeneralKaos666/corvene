//! Corvene `1212-bisect` (GitHub Desktop has no bisect): starting a bisect
//! with uncommitted changes stashes them on the current branch first, as
//! `StashAndSwitchBranch` does for a branch switch.

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::sizes::*;

/// `Popup::StartBisect`
pub struct StartBisectDialog {
    repo: u64,
    mark: Option<(corvene_git::BisectVerdict, String)>,
    branch: String,
}

impl StartBisectDialog {
    pub fn new(
        repo: u64,
        mark: Option<(corvene_git::BisectVerdict, String)>,
        branch: String,
    ) -> Self {
        Self { repo, mark, branch }
    }
}

impl Render for StartBisectDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, mark) = (self.repo, self.mark.clone());
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(crate::widgets::paragraph(vec![
                "Bisecting checks out other commits, so your changes will be stashed on ".into(),
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.branch.clone())
                    .into_any_element()
                    .into(),
                " first.".into(),
            ]))
            .child("Restore them from Stashed Changes once you stop bisecting.");
        dialog(
            "dialog-start-bisect",
            mac_or("Start Bisect", "Start bisect"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "start-bisect-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "start-bisect-ok",
                    label: mac_or("Stash Changes and Start", "Stash changes and start").into(),
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::start_bisect(repo, mark.clone(), true, cx);
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
