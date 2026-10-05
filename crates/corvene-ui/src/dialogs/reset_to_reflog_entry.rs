//! Corvene `1216-recent-activity`: Recent Activity › Reset Current Branch
//! to Here…. GitHub Desktop has no reflog view; the closest dialog is
//! `ui/reset/warning-before-reset.tsx`, which warns before a mixed reset.
//! This one resets hard (the point is to get an earlier state back, files
//! included), so uncommitted changes are stashed on the branch first; on a
//! detached HEAD, where Desktop cannot stash, they have to be dealt with
//! first.

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog_with_kind};
use crate::theme::sizes::*;

pub struct ResetToReflogEntryDialog {
    repo: u64,
    sha: String,
    branch: Option<String>,
    dirty: usize,
}

impl ResetToReflogEntryDialog {
    pub fn new(repo: u64, sha: String, branch: Option<String>, dirty: usize) -> Self {
        Self {
            repo,
            sha,
            branch,
            dirty,
        }
    }
}

impl Render for ResetToReflogEntryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, sha) = (self.repo, self.sha.clone());
        let short = &self.sha[..self.sha.len().min(7)];
        let target = match &self.branch {
            Some(branch) => format!("{branch} now points"),
            None => "HEAD now points".to_string(),
        };
        let stash = self.dirty > 0 && self.branch.is_some();
        let blocked = self.dirty > 0 && self.branch.is_none();
        let changes = match self.dirty {
            1 => "1 changed file".to_string(),
            n => format!("{n} changed files"),
        };
        let mut body = div().flex().flex_col().gap(SPACING()).child(format!(
            "The working tree will match commit {short}. The commit {target} at stays in \
             Recent Activity, so this can be undone the same way."
        ));
        if stash {
            body = body.child(format!(
                "Your {changes} will be stashed on {} first.",
                self.branch.as_deref().unwrap_or_default()
            ));
        } else if blocked {
            body = body.child(format!(
                "You have {changes}. Commit or discard them first: changes can only be stashed \
                 on a branch."
            ));
        }
        dialog_with_kind(
            "dialog-reset-to-reflog-entry",
            DialogKind::Warning,
            match &self.branch {
                Some(branch) => format!("Reset {branch} to {short}?"),
                None => format!("Move HEAD to {short}?"),
            },
            body,
            OkCancelButtonGroup {
                destructive: !stash,
                cancel: GroupButtonSpec {
                    id: "reflog-reset-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "reflog-reset-ok",
                    label: if stash {
                        mac_or("Stash and Reset", "Stash and reset").into()
                    } else {
                        "Reset".into()
                    },
                    disabled: blocked,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::reflog_reset(repo, sha.clone(), stash, cx);
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
