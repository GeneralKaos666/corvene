//! GHD `HookFailed` (`ui/hook-failed/hook-failed.tsx`) and `CommitProgress`
//! (`ui/commit-progress/commit-progress.tsx`): a failed hook's output with
//! Abort / Ignore and Continue, and the live output of the commit in
//! progress. Both show it in GHD's 80-column `Terminal`
//! ([`crate::terminal`]).

use corvene_core::hooks::{CommitOutput, HookFailureReply};
use corvene_core::{AppState, Dispatcher, Popup};
use corvene_git::hooks::HookFailureResolution;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{
    DialogButton, DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog, dialog_with_kind,
};
use crate::terminal::Terminal;
use crate::theme::sizes::*;

/// GHD `HookFailed`.
pub struct HookFailedDialog {
    hook_name: String,
    reply: HookFailureReply,
    terminal: Entity<Terminal>,
}

impl HookFailedDialog {
    pub fn new(
        hook_name: String,
        terminal_output: &str,
        reply: HookFailureReply,
        cx: &mut Context<Self>,
    ) -> Self {
        // `<Terminal terminalOutput rows={15} cols={80} />`
        let terminal = cx.new(|_| Terminal::new(80, 15, terminal_output.as_bytes()));
        Self {
            hook_name,
            reply,
            terminal,
        }
    }
}

impl Render for HookFailedDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hook_name = self.hook_name.clone();
        let close = |reply: HookFailureReply| {
            move |_: &mut Window, cx: &mut App| {
                let reply = reply.clone();
                Dispatcher::close_popups_where(
                    move |p| matches!(p, Popup::HookFailed { reply: r, .. } if *r == reply),
                    cx,
                );
            }
        };
        // `onDismissed`: `resolve('abort')` (`closed_popup`)
        let abort = close(self.reply.clone());
        let ignore = {
            let reply = self.reply.clone();
            let close = close(self.reply.clone());
            move |window: &mut Window, cx: &mut App| {
                reply.send(HookFailureResolution::Ignore);
                close(window, cx);
            }
        };
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(div().id("hook-failure-message").child(format!(
                "The {hook_name} hook failed. What would you like to do?"
            )))
            .child(self.terminal.clone());
        dialog_with_kind(
            "hook-failed-dialog",
            DialogKind::Warning,
            format!("{hook_name} {}", mac_or("Failed", "failed")),
            content,
            OkCancelButtonGroup {
                destructive: true,
                cancel: GroupButtonSpec {
                    id: "hook-failed-abort",
                    label: "Abort".into(),
                    disabled: false,
                    on_click: Box::new(abort.clone()),
                },
                ok: GroupButtonSpec {
                    id: "hook-failed-ignore",
                    label: "Ignore and Continue".into(),
                    disabled: false,
                    on_click: Box::new(ignore),
                },
            }
            .into_buttons(),
            abort,
            window,
            cx,
        )
    }
}

/// GHD `CommitProgress`: subscribed to the commit's output while it is
/// open; what came in stays once the commit is done.
pub struct CommitProgressDialog {
    state: Entity<AppState>,
    output: CommitOutput,
    /// How much of `output` the terminal has.
    offset: usize,
    terminal: Entity<Terminal>,
    _observe: Subscription,
}

impl CommitProgressDialog {
    pub fn new(state: Entity<AppState>, output: CommitOutput, cx: &mut Context<Self>) -> Self {
        // `subscribeToCommitOutput`: what was written so far, then the rest
        let (written, offset) = output.tail();
        // `<Terminal hideCursor cols={80} rows={20} />`
        let terminal = cx.new(|_| Terminal::new(80, 20, &written));
        let _observe = cx.observe(&state, |this: &mut Self, _, cx| this.catch_up(cx));
        Self {
            state,
            output,
            offset,
            terminal,
            _observe,
        }
    }

    fn catch_up(&mut self, cx: &mut Context<Self>) {
        let (more, offset) = self.output.read_from(self.offset);
        self.offset = offset;
        if !more.is_empty() {
            self.terminal.update(cx, |t, cx| t.write(&more, cx));
        }
    }
}

impl Render for CommitProgressDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let output = self.output.clone();
        let state = self.state.clone();
        let close = move |window: &mut Window, cx: &mut App| {
            let output = output.clone();
            Dispatcher::close_popups_where(
                move |p| matches!(p, Popup::CommitProgress { output: o } if *o == output),
                cx,
            );
            // `App.onPopupDismissed`: once the commit is done, focus goes
            // back to the commit button (deferred past the workspace's
            // focus restore on popup close)
            let committing = state
                .read(cx)
                .selected_state()
                .is_some_and(|r| r.committing);
            if !committing {
                window.defer(cx, crate::changes::focus_commit_button);
            }
        };
        dialog(
            "commit-progress-dialog",
            "Committing changes",
            self.terminal.clone(),
            vec![DialogButton {
                id: "commit-progress-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close.clone()),
            }],
            close,
            window,
            cx,
        )
    }
}
