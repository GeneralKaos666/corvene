//! Delete Branches: Corvene's confirmation for deleting the branch list's
//! multi-selection (`895-bulk-delete-branches`). GHD
//! (`ui/delete-branch/delete-branch-dialog.tsx`) confirms one branch at a
//! time; this one lists every selected branch with what deleting it would
//! lose and offers to delete the remote branches too.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvene_core::delete_branches::{DeleteBranchesPreview, is_old, upstream_gone};
use corvene_core::{AppState, Branch, BranchKind, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogFrame, DialogKind};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::checkbox;

pub struct DeleteBranchesDialog {
    state: Entity<AppState>,
    repo: u64,
    names: Vec<String>,
    include_remote: bool,
    scroll: ScrollHandle,
}

impl DeleteBranchesDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        names: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Dispatcher::preview_delete_branches(repo, names.clone(), cx);
        Self {
            state,
            repo,
            names,
            include_remote: false,
            scroll: ScrollHandle::new(),
        }
    }
}

/// One listed branch: its name, whether its remote branch can be deleted
/// too, and its marks (a mark is a warning when it means lost commits).
struct Row {
    name: String,
    remote: Option<String>,
    marks: Vec<(String, bool)>,
}

/// The upstream (short name) a local branch's "delete on the remote" would
/// delete: it must exist and not be the remote's default branch
/// (`870-delete-remote-names-upstream`'s rule, always applied here).
fn deletable_upstream(
    branch: &Branch,
    branches: &[Branch],
    default_branch: Option<&str>,
) -> Option<String> {
    let upstream = branch.upstream_short()?;
    if !branches
        .iter()
        .any(|b| b.kind == BranchKind::Remote && b.name == upstream)
    {
        return None;
    }
    let default_upstream = default_branch
        .and_then(|d| {
            branches
                .iter()
                .find(|b| b.name == d && b.kind == BranchKind::Local)
        })
        .and_then(|b| b.upstream_short());
    let remote_default = default_upstream == Some(upstream)
        || default_branch
            .is_some_and(|d| upstream.split_once('/').is_some_and(|(_, name)| name == d));
    (!remote_default).then(|| upstream.to_string())
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn rows(
    names: &[String],
    branches: &[Branch],
    default_branch: Option<&str>,
    preview: Option<&DeleteBranchesPreview>,
) -> Vec<Row> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    names
        .iter()
        .filter_map(|name| {
            branches
                .iter()
                .find(|b| &b.name == name && b.kind == BranchKind::Local)
        })
        .map(|b| {
            let mut marks = Vec::new();
            if Some(b.name.as_str()) == default_branch {
                marks.push(("default branch".to_string(), true));
            }
            if let Some(p) = preview {
                if let Some(n) = p.unmerged.get(&b.name).copied().filter(|n| *n > 0) {
                    marks.push((
                        format!(
                            "{} not on {}",
                            plural(n, "commit", "commits"),
                            p.default_branch.as_deref().unwrap_or("the default branch")
                        ),
                        true,
                    ));
                }
                if let Some(n) = p.unpushed.get(&b.name).copied().filter(|n| *n > 0) {
                    marks.push((plural(n, "unpushed commit", "unpushed commits"), true));
                }
            }
            if b.upstream.is_none() {
                marks.push(("not published".to_string(), false));
            } else if upstream_gone(b, branches) {
                marks.push(("deleted on the remote".to_string(), false));
            }
            if is_old(b, now)
                && let Some(t) = b.tip_time
            {
                marks.push((
                    format!(
                        "last commit {}",
                        relative(UNIX_EPOCH + Duration::from_secs(t.max(0) as u64))
                    ),
                    false,
                ));
            }
            Row {
                name: b.name.clone(),
                remote: deletable_upstream(b, branches, default_branch),
                marks,
            }
        })
        .collect()
}

impl Render for DeleteBranchesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd().clone();
        let (rows, undo) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let branches = rs
                .and_then(|r| r.info.as_ref())
                .map(|i| i.branches.as_slice())
                .unwrap_or_default();
            let preview = rs
                .and_then(|r| r.delete_branches_preview.as_ref())
                .filter(|p| p.branches == self.names);
            (
                rows(
                    &self.names,
                    branches,
                    rs.and_then(|r| r.default_branch.as_deref()),
                    preview,
                ),
                s.flags.bool(corvene_core::flags::ids::UNDO_DELETE_BRANCH),
            )
        };
        let remote_count = rows.iter().filter(|r| r.remote.is_some()).count();
        let targets: Vec<(String, bool)> = rows
            .iter()
            .map(|r| (r.name.clone(), self.include_remote && r.remote.is_some()))
            .collect();
        let count = rows.len();
        let repo = self.repo;
        let list = div()
            .id("delete-branches-list")
            .max_h(zpx(220.))
            .overflow_y_scroll()
            .mb(SPACING())
            .border_1()
            .border_color(t.box_border)
            .rounded(zpx(4.))
            .flex()
            .flex_col()
            .children(rows.iter().enumerate().map(|(ix, row)| {
                div()
                    .id(("delete-branches-row", ix))
                    .px(SPACING())
                    .py(SPACING_HALF())
                    .when(ix > 0, |d| d.border_t_1().border_color(t.box_border))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .child(octicon(Octicon::GitBranch, t.text_secondary))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(row.name.clone()),
                            ),
                    )
                    .when(!row.marks.is_empty(), |d| {
                        d.child(
                            div()
                                .pl(zpx(20.))
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_x(SPACING_HALF())
                                .text_size(FONT_SIZE_SM())
                                .children(row.marks.iter().enumerate().map(
                                    |(i, (mark, warning))| {
                                        div()
                                            .text_color(if *warning {
                                                t.dialog_warning
                                            } else {
                                                t.text_secondary
                                            })
                                            .child(if i + 1 < row.marks.len() {
                                                format!("{mark} ·")
                                            } else {
                                                mark.clone()
                                            })
                                    },
                                )),
                        )
                    })
            }))
            .with_scrollbar_handle(&self.scroll);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING()).child(format!(
                "Delete {count} {}?",
                if count == 1 { "branch" } else { "branches" }
            )))
            .child(list)
            .child(
                div()
                    .when(remote_count > 0, |d| d.mb(SPACING()))
                    .child(if undo {
                        "Deleted local branches can be restored with Undo right afterwards."
                    } else {
                        "This action cannot be undone."
                    }),
            )
            .when(remote_count > 0, |d| {
                d.child(
                    div()
                        .id("delete-branches-remote-row")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.include_remote = !this.include_remote;
                            cx.notify();
                        }))
                        .child(checkbox(
                            "delete-branches-remote-box",
                            self.include_remote,
                            false,
                            cx,
                        ))
                        .child(format!(
                            "Also delete the {} on the remote",
                            if remote_count == 1 {
                                "branch that exists".to_string()
                            } else {
                                format!("{remote_count} branches that exist")
                            }
                        )),
                )
                .when(self.include_remote, |d| {
                    d.child(
                        div()
                            .mt(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child("Branches deleted on the remote cannot be restored."),
                    )
                })
            });
        crate::dialog::dialog_with_kind_framed(
            "dialog-delete-branches",
            DialogKind::Warning,
            mac_or("Delete Branches", "Delete branches"),
            content,
            vec![
                DialogButton {
                    id: "delete-branches-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "delete-branches-ok",
                    label: if count == 1 {
                        mac_or("Delete Branch", "Delete branch").into()
                    } else if cfg!(target_os = "macos") {
                        format!("Delete {count} Branches").into()
                    } else {
                        format!("Delete {count} branches").into()
                    },
                    primary: false,
                    disabled: count == 0,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::delete_branches(repo, targets.clone(), cx);
                    }),
                },
            ],
            DialogFrame {
                focus_primary: true,
                ..DialogFrame::default()
            },
            close,
            window,
            cx,
        )
    }
}
