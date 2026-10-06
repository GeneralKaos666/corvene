//! Corvene `1216-recent-activity`: the History sidebar while Repository ›
//! Recent Activity… is open (`corvene_core::reflog`). GitHub Desktop has no
//! reflog view.
//!
//! A header names the log (HEAD or the current branch) with an options menu
//! (which log, Show Every Step, Only Unreachable) and Close. Each row says
//! what happened in plain words, when, and the commit it left HEAD at; rows
//! whose commit no branch or tag reaches are marked Unreachable, and a row
//! that switched away from a branch since deleted says so. Selecting a row
//! shows its commit in the usual commit view; its context menu creates a
//! branch there, resets the current branch to it, restores the deleted
//! branch, or copies the SHA.

use std::time::{Duration, SystemTime};

use corvene_core::reflog::{ReflogSource, ReflogState};
use corvene_core::{AppState, Dispatcher, Popup, Tip};
use corvene_git::{CommitKind, RebaseStep, ReflogAction, ReflogEntry};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{MenuItem, mac_or};
use crate::history::commit_row_height;
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative_at;
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};
use crate::widgets::{GhdTooltip, IconButtonA11y};

pub struct ReflogList {
    state: Entity<AppState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    /// The selected entry last scrolled into view.
    scrolled_to: Option<usize>,
    /// Focused since it opened (arrows and Escape work at once).
    focused: bool,
}

impl ReflogList {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle().tab_stop(true),
            scrolled_to: None,
            focused: false,
        }
    }

    /// ↑ / ↓: the next row up or down that has a commit.
    fn move_selection(&mut self, id: u64, down: bool, cx: &mut Context<Self>) {
        let next = {
            let s = self.state.read(cx);
            let Some(r) = s.repo_states.get(&id).and_then(|rs| rs.reflog.as_ref()) else {
                return;
            };
            let current = r.selected.and_then(|e| r.row_of(e));
            let candidates: Box<dyn Iterator<Item = usize>> = match (current, down) {
                (None, _) => Box::new(0..r.rows.len()),
                (Some(c), true) => Box::new(c + 1..r.rows.len()),
                (Some(c), false) => Box::new((0..c).rev()),
            };
            candidates
                .map(|row| r.rows[row].entry)
                .find(|&e| r.has_commit(e))
        };
        if let Some(entry) = next {
            Dispatcher::select_reflog_entry(id, entry, cx);
        }
    }

    fn header(&self, id: u64, r: &ReflogState, branch: Option<String>, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let source = match (&r.source, &r.branch) {
            (ReflogSource::CurrentBranch, Some(b)) => b.clone(),
            _ => "HEAD".to_string(),
        };
        let (source_kind, show_steps, only_unreachable) =
            (r.source, r.show_steps, r.only_unreachable);
        let button = |el: Stateful<Div>| {
            el.h(zpx(19.))
                .px(SPACING_HALF())
                .flex()
                .flex_row()
                .items_center()
                .gap(zpx(2.))
                .rounded(BORDER_RADIUS())
                .cursor_pointer()
                .text_size(FONT_SIZE_SM())
                .text_color(t.text)
                .hover(|d| d.bg(t.box_hover_background))
        };
        div()
            .h(zpx(36.))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING_HALF())
            .pl(SPACING())
            .pr(SPACING_HALF())
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE())
            .child(octicon(Octicon::History, t.text_secondary).flex_none())
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(mac_or("Recent Activity", "Recent activity")),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_color(t.text_secondary)
                    .child(source),
            )
            .child(div().flex_1())
            .child(
                button(div().id("reflog-options"))
                    .icon_button_label("Recent Activity Options")
                    .child(octicon(Octicon::Gear, t.text))
                    .child(octicon(Octicon::TriangleDown, t.text))
                    .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                        cx.stop_propagation();
                        options_menu(
                            id,
                            source_kind,
                            branch.clone(),
                            show_steps,
                            only_unreachable,
                            ev.position,
                            window,
                            cx,
                        );
                    }),
            )
            .child(
                button(div().id("reflog-close"))
                    .icon_button_label("Close Recent Activity")
                    .child(octicon(Octicon::X, t.text))
                    .on_click(move |_, _, cx| Dispatcher::close_recent_activity(id, cx)),
            )
    }

    fn message(text: &str, t: &GhdTheme) -> Div {
        div()
            .flex_1()
            .flex()
            .items_start()
            .justify_center()
            .pt(SPACING() * 2.)
            .px(SPACING())
            .text_color(t.text_secondary)
            .child(text.to_string())
    }
}

/// Which log, and the two filters, as a menu of checkboxes.
#[allow(clippy::too_many_arguments)]
fn options_menu(
    id: u64,
    source: ReflogSource,
    branch: Option<String>,
    show_steps: bool,
    only_unreachable: bool,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut items = vec![MenuItem::checkbox(
        "HEAD",
        source == ReflogSource::Head,
        move |_, cx| Dispatcher::set_reflog_source(id, ReflogSource::Head, cx),
    )];
    if let Some(branch) = branch {
        items.push(MenuItem::checkbox(
            format!("Branch {branch}"),
            source == ReflogSource::CurrentBranch,
            move |_, cx| Dispatcher::set_reflog_source(id, ReflogSource::CurrentBranch, cx),
        ));
    }
    items.extend([
        MenuItem::separator(),
        MenuItem::checkbox(
            mac_or("Show Every Step", "Show every step"),
            show_steps,
            move |_, cx| Dispatcher::set_reflog_options(id, !show_steps, only_unreachable, cx),
        ),
        MenuItem::checkbox(
            mac_or("Only Unreachable", "Only unreachable"),
            only_unreachable,
            move |_, cx| Dispatcher::set_reflog_options(id, show_steps, !only_unreachable, cx),
        ),
    ]);
    crate::native_menu::show_context_menu(items, position, window, cx);
}

/// What a row's context menu acts on.
#[derive(Clone)]
struct RowTarget {
    sha: String,
    has_commit: bool,
    /// The commit HEAD is at: resetting there would do nothing.
    is_head: bool,
    branch: Option<String>,
    /// A deleted branch and the tip it had.
    deleted: Option<(String, String)>,
}

fn row_menu(
    id: u64,
    target: RowTarget,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let RowTarget {
        sha,
        has_commit,
        is_head,
        branch,
        deleted,
    } = target;
    let mut items = Vec::new();
    if let Some((name, tip)) = deleted {
        items.push(MenuItem::new(
            format!("Restore Branch \u{201c}{name}\u{201d}"),
            move |_, cx| Dispatcher::restore_deleted_branch(id, name.clone(), tip.clone(), cx),
        ));
        items.push(MenuItem::separator());
    }
    let branch_sha = sha.clone();
    items.push(
        MenuItem::new(
            mac_or("Create Branch Here…", "Create branch here…"),
            move |_, cx| {
                Dispatcher::show_popup(
                    Popup::CreateBranch {
                        repo: id,
                        target_sha: Some(branch_sha.clone()),
                        initial_name: String::new(),
                    },
                    cx,
                )
            },
        )
        .enabled(has_commit),
    );
    let reset_sha = sha.clone();
    items.push(
        MenuItem::new(
            match &branch {
                Some(b) => format!("Reset {b} to {}", mac_or("Here…", "here…")),
                None => mac_or("Move HEAD Here…", "Move HEAD here…").to_string(),
            },
            move |_, cx| Dispatcher::request_reflog_reset(id, reset_sha.clone(), cx),
        )
        .enabled(has_commit && !is_head),
    );
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Copy SHA", move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()))
    }));
    crate::native_menu::show_context_menu(items, position, window, cx);
}

fn short(sha_or_name: &str) -> &str {
    let is_sha = sha_or_name.len() == 40 && sha_or_name.bytes().all(|b| b.is_ascii_hexdigit());
    if is_sha {
        &sha_or_name[..7]
    } else {
        sha_or_name
    }
}

/// What happened, in plain words. `folded` rebase steps are counted.
pub fn entry_label(entry: &ReflogEntry, folded: usize) -> String {
    let steps = match folded {
        0 => String::new(),
        1 => " (1 step)".to_string(),
        n => format!(" ({n} steps)"),
    };
    match &entry.action {
        ReflogAction::Commit { kind, summary } => match kind {
            CommitKind::Normal => format!("Commit: {summary}"),
            CommitKind::Initial => format!("First commit: {summary}"),
            CommitKind::Amend => format!("Amended: {summary}"),
            CommitKind::Merge => format!("Merge commit: {summary}"),
        },
        ReflogAction::Checkout { from, to } => {
            format!("Switched from {} to {}", short(from), short(to))
        }
        ReflogAction::Reset { target } => format!("Reset to {}", short(target)),
        ReflogAction::Rebase { step, detail } => match step {
            RebaseStep::Start => format!("Rebase started onto {}", short(detail)),
            RebaseStep::Pick => format!("Rebase step: {detail}"),
            RebaseStep::Finish => format!("Rebased {detail}{steps}"),
            RebaseStep::Abort => format!("Rebase of {detail} aborted{steps}"),
        },
        ReflogAction::Merge { what, fast_forward } => {
            if *fast_forward {
                format!("Fast-forwarded to {what}")
            } else {
                format!("Merged {what}")
            }
        }
        ReflogAction::Pull { fast_forward } => {
            if *fast_forward {
                "Pulled (fast-forward)".to_string()
            } else {
                "Pulled and merged".to_string()
            }
        }
        ReflogAction::CherryPick { summary } => format!("Cherry-picked: {summary}"),
        ReflogAction::Revert { summary } => format!("Commit: {summary}"),
        ReflogAction::Branch { detail } => match detail.strip_prefix("Created from ") {
            Some(from) => format!("Branch created from {}", short(from)),
            None => format!("Branch: {detail}"),
        },
        ReflogAction::Clone { url } => format!("Cloned from {url}"),
        ReflogAction::Other if entry.message.is_empty() => "(no message)".to_string(),
        ReflogAction::Other => entry.message.clone(),
    }
}

/// A small outlined label after the time; a long one is cut short.
fn pill(text: String, color: Hsla) -> Div {
    div()
        .min_w_0()
        .flex_shrink(1.)
        .h(zpx(15.))
        .px(SPACING_THIRD())
        .flex()
        .items_center()
        .rounded(BORDER_RADIUS())
        .border_1()
        .border_color(color)
        .text_color(color)
        .text_size(FONT_SIZE_SM())
        .line_height(zpx(13.))
        .child(div().min_w_0().truncate().child(text))
}

impl Render for ReflogList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = {
            let s = self.state.read(cx);
            s.selected.and_then(|id| {
                let rs = s.repo_states.get(&id)?;
                let r = rs.reflog.clone()?;
                let (branch, head) = match rs.info.as_ref().map(|i| &i.tip) {
                    Some(Tip::Valid { branch }) => (Some(branch.name.clone()), branch.tip.clone()),
                    Some(Tip::Detached { sha }) => (None, Some(sha.clone())),
                    _ => (None, None),
                };
                Some((id, r, branch, head))
            })
        };
        let Some((id, r, branch, head)) = loaded else {
            self.focused = false;
            return div().into_any_element();
        };
        if !self.focused {
            self.focused = true;
            self.scrolled_to = None;
            window.focus(&self.focus, cx);
        }
        let t = cx.ghd().clone();
        let header = self.header(id, &r, branch.clone(), cx);

        // keep the selected row on screen (keyboard, reloads)
        if r.selected != self.scrolled_to {
            self.scrolled_to = r.selected;
            if let Some(row) = r.selected.and_then(|e| r.row_of(e)) {
                self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
            }
        }
        let list_focused = self.focus.contains_focused(window, cx);
        let body = if !r.loaded {
            Self::message("", &t).into_any_element()
        } else if r.rows.is_empty() {
            let text = if r.only_unreachable {
                "Every commit here is still on a branch or tag"
            } else {
                "No recent activity"
            };
            Self::message(text, &t).into_any_element()
        } else {
            let row_height = commit_row_height(cx);
            let now = SystemTime::now();
            let focus = self.focus.clone();
            uniform_list("reflog-rows", r.rows.len(), move |range, _window, _cx| {
                range
                    .map(|ix| {
                        let row = r.rows[ix];
                        let entry = &r.entries[row.entry];
                        let has_commit = r.has_commit(row.entry);
                        let unreachable = r.is_unreachable(row.entry);
                        let deleted = r.deleted_branches.get(&row.entry).cloned();
                        let selected = r.selected == Some(row.entry);
                        let (bg, text, secondary) = if selected && list_focused {
                            (
                                t.box_selected_active_background,
                                t.box_selected_active_text,
                                t.box_selected_active_text,
                            )
                        } else if selected {
                            (
                                t.box_selected_background,
                                t.box_selected_text,
                                t.box_selected_text,
                            )
                        } else {
                            (t.background, t.text, t.text_secondary)
                        };
                        let when = SystemTime::UNIX_EPOCH
                            + Duration::from_secs(entry.seconds.max(0) as u64);
                        let mut byline = relative_at(when, now);
                        if !entry.new.is_empty() {
                            byline.push_str(" • ");
                            byline.push_str(&entry.new[..entry.new.len().min(7)]);
                        }
                        let target = RowTarget {
                            sha: entry.new.clone(),
                            has_commit,
                            is_head: head.as_deref() == Some(entry.new.as_str()),
                            branch: branch.clone(),
                            deleted: deleted.clone().map(|name| (name, entry.old.clone())),
                        };
                        let entry_ix = row.entry;
                        let (focus, menu_focus) = (focus.clone(), focus.clone());
                        div()
                            .id(("reflog-row", ix))
                            .ghd_tooltip(entry.message.clone())
                            .h(row_height)
                            .w_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(SPACING())
                            .pr(SPACING() + SPACING_HALF())
                            .bg(bg)
                            .border_b_1()
                            .border_color(t.box_border)
                            .when(!has_commit, |d| d.opacity(0.6))
                            .when(has_commit, |d| {
                                d.on_click(move |_, window, cx| {
                                    window.focus(&focus, cx);
                                    Dispatcher::select_reflog_entry(id, entry_ix, cx);
                                })
                            })
                            .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
                                cx.stop_propagation();
                                window.focus(&menu_focus, cx);
                                Dispatcher::select_reflog_entry(id, entry_ix, cx);
                                row_menu(id, target.clone(), ev.position, window, cx);
                            })
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(FONT_SIZE())
                                    .line_height(zpx(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text)
                                    .child(entry_label(entry, row.folded)),
                            )
                            .child(
                                div()
                                    .mt(zpx(3.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(SPACING_HALF())
                                    .text_size(FONT_SIZE_SM())
                                    .line_height(zpx(16.5))
                                    .text_color(secondary)
                                    .child(div().flex_none().child(byline))
                                    .when(!has_commit, |d| {
                                        d.child(pill("Gone".to_string(), secondary))
                                    })
                                    .when(has_commit && unreachable, |d| {
                                        let color = if selected {
                                            secondary
                                        } else {
                                            t.dialog_warning
                                        };
                                        d.child(pill("Unreachable".to_string(), color))
                                    })
                                    .when(deleted.is_some(), |d| {
                                        d.child(pill("Branch deleted".to_string(), secondary))
                                    }),
                            )
                    })
                    .collect()
            })
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
            .with_scrollbar_handle(&self.scroll)
            .into_any_element()
        };

        let view = cx.entity().downgrade();
        let t = cx.ghd();
        div()
            .id("reflog-list")
            .track_focus(&self.focus)
            .on_key_down(move |ev, _, cx| {
                let down = match ev.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        Dispatcher::close_recent_activity(id, cx);
                        return;
                    }
                    "down" => true,
                    "up" => false,
                    _ => return,
                };
                cx.stop_propagation();
                view.update(cx, |this, cx| this.move_selection(id, down, cx))
                    .ok();
            })
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(t.background)
            .child(header)
            .child(body)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(message: &str, folded: usize) -> String {
        entry_label(
            &ReflogEntry {
                old: String::new(),
                new: String::new(),
                seconds: 0,
                offset: 0,
                message: message.to_string(),
                action: corvene_git::parse_reflog_action(message),
            },
            folded,
        )
    }

    #[::core::prelude::v1::test]
    fn labels_in_plain_words() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(label("reset: moving to HEAD~3", 0), "Reset to HEAD~3");
        assert_eq!(
            label(&format!("checkout: moving from {sha} to main"), 0),
            "Switched from 0123456 to main"
        );
        assert_eq!(
            label("rebase (finish): returning to refs/heads/topic", 4),
            "Rebased topic (4 steps)"
        );
        assert_eq!(label("commit (amend): Fix", 0), "Amended: Fix");
        assert_eq!(label("update by push", 0), "update by push");
    }
}
