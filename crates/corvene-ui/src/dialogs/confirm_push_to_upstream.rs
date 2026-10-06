//! Corvene (flag `1222-push-target-guard`, desktop/desktop#21021): Push of a
//! branch whose upstream is the remote's default branch under another name
//! (`issue-134` tracking `origin/main`) asks first. GHD pushes
//! `issue-134:main` without asking (`lib/git/push.ts`).

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogFrame, DialogKind, dialog_with_kind_framed};
use crate::theme::sizes::*;
use crate::widgets::{code_ref, paragraph};

pub struct ConfirmPushToUpstreamDialog {
    repo: u64,
    branch: String,
    upstream: String,
    remote_branch: String,
    force_with_lease: bool,
    up_to: Option<String>,
}

impl ConfirmPushToUpstreamDialog {
    pub fn new(
        repo: u64,
        branch: String,
        upstream: String,
        remote_branch: String,
        force_with_lease: bool,
        up_to: Option<String>,
    ) -> Self {
        Self {
            repo,
            branch,
            upstream,
            remote_branch,
            force_with_lease,
            up_to,
        }
    }
}

impl Render for ConfirmPushToUpstreamDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repo = self.repo;
        let remote = self
            .upstream
            .strip_suffix(&self.remote_branch)
            .and_then(|r| r.strip_suffix('/'))
            .unwrap_or(&self.upstream)
            .to_string();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(vec![
                code_ref(self.branch.clone(), cx).into_any_element().into(),
                " tracks ".into(),
                code_ref(self.upstream.clone(), cx)
                    .into_any_element()
                    .into(),
                format!(
                    ", the default branch of {remote}. Pushing sends its commits to {}.",
                    self.remote_branch
                )
                .into(),
            ]))
            .child(paragraph(vec![
                format!(
                    "Publish it as {} on {remote} to keep its commits on a branch of their own.",
                    self.branch
                )
                .into(),
            ]));
        let upstream = self.upstream.clone();
        let branch = self.branch.clone();
        let push_branch = self.branch.clone();
        let (force_with_lease, up_to) = (self.force_with_lease, self.up_to.clone());
        let buttons = vec![
            DialogButton {
                id: "push-to-upstream-cancel",
                label: "Cancel".into(),
                primary: false,
                disabled: false,
                on_click: Box::new(close),
            },
            DialogButton {
                id: "push-to-upstream-push",
                label: format!("Push to {}", self.remote_branch).into(),
                primary: false,
                disabled: false,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::confirm_push_to_upstream(
                        repo,
                        push_branch.clone(),
                        upstream.clone(),
                        force_with_lease,
                        up_to.clone(),
                        cx,
                    )
                }),
            },
            DialogButton {
                id: "push-to-upstream-publish",
                label: mac_or("Publish Branch", "Publish branch").into(),
                primary: true,
                disabled: false,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::publish_under_own_name(repo, branch.clone(), cx)
                }),
            },
        ];
        dialog_with_kind_framed(
            "dialog-confirm-push-to-upstream",
            DialogKind::Warning,
            format!("Push to {}?", self.remote_branch),
            content,
            buttons,
            DialogFrame {
                focus_primary: true,
                ..Default::default()
            },
            close,
            window,
            cx,
        )
    }
}
