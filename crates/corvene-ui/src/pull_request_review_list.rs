//! Corvene `348-pull-request-review`: the History sidebar while a pull
//! request is under review (`corvene_core::pull_request_review`). GitHub
//! Desktop has no such list: its pull request list only checks a pull
//! request out.
//!
//! A header names the pull request (`#N title`) and offers a refresh, View
//! on GitHub and Close; under it an Overview row, then the changed files
//! as the commit file list draws them (status icon, path) with the count
//! of their unresolved threads (pending ones in yellow); a footer shows
//! what the pending review holds and its Review changes… button. ↑ / ↓
//! move through the rows, Escape closes the review.

use corvene_core::pull_request_review::{PullRequestReviewState, ReviewSelection};
use corvene_core::{AppState, CommittedFileChange, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::github_list::{self, header, header_button};
use crate::icons::{Octicon, octicon, spin};
use crate::review_threads::thread_badge;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{GhdTooltip, ListRowA11y, primary_button};

/// GHD's commit file list row (`.file-list .list-item`).
fn row_height() -> Pixels {
    zpx(29.)
}

/// What one render reads from the review state.
struct ListSnapshot {
    number: u64,
    title: String,
    draft: bool,
    busy_loading: bool,
    preparing: bool,
    prepare_error: Option<String>,
    busy: Option<String>,
    error: Option<String>,
    composer_open: bool,
    signed_out: bool,
    pending: u64,
    unresolved: usize,
    selection: ReviewSelection,
    files: Vec<CommittedFileChange>,
    counts: std::collections::HashMap<String, corvene_core::pull_request_review::FileThreadCounts>,
}

impl ListSnapshot {
    fn of(review: &PullRequestReviewState) -> Self {
        Self {
            number: review.number(),
            title: review.pull_request.title.clone(),
            draft: review.pull_request.draft,
            busy_loading: review.threads_loading || review.overview_loading || review.preparing,
            preparing: review.preparing,
            prepare_error: review.prepare_error.clone(),
            busy: review.busy.clone(),
            error: review.error.clone(),
            composer_open: review.composer.is_some(),
            signed_out: review.signed_out,
            pending: review.pending_count(),
            unresolved: review.unresolved_count(),
            selection: review.selection.clone(),
            files: review
                .changeset
                .as_ref()
                .map(|c| c.files.clone())
                .unwrap_or_default(),
            counts: review
                .counts_by_file()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }
}

pub struct PullRequestReviewList {
    state: Entity<AppState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    /// Focused since it opened (arrows and Escape work at once).
    focused: bool,
}

impl PullRequestReviewList {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            focused: false,
        }
    }

    /// ↑ / ↓: the previous / next row (Overview first).
    fn move_selection(&mut self, id: u64, down: bool, cx: &mut Context<Self>) {
        let (paths, current) = {
            let s = self.state.read(cx);
            let Some(review) = crate::review_threads::review_state_of(s, id) else {
                return;
            };
            let paths: Vec<String> = review
                .changeset
                .as_ref()
                .map(|c| c.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            let current = match &review.selection {
                ReviewSelection::Overview => 0,
                ReviewSelection::File(p) => paths.iter().position(|x| x == p).map_or(0, |i| i + 1),
            };
            (paths, current)
        };
        let next = if down {
            (current + 1).min(paths.len())
        } else {
            current.saturating_sub(1)
        };
        if next == current {
            return;
        }
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        if next == 0 {
            Dispatcher::select_review_overview(id, cx);
        } else {
            Dispatcher::select_review_file(id, paths[next - 1].clone(), cx);
        }
    }

    fn header(&self, id: u64, review: &ListSnapshot, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let busy = review.busy_loading;
        let refresh = header_button("review-refresh", "Refresh the pull request", cx)
            .ghd_tooltip("Refresh")
            .child(if busy {
                spin(
                    octicon(Octicon::SyncClockwise, t.text),
                    "review-refresh-spin",
                )
                .into_any_element()
            } else {
                octicon(Octicon::SyncClockwise, t.text).into_any_element()
            })
            .on_click(move |_, _, cx| Dispatcher::refresh_pull_request_review(id, cx))
            .into_any_element();
        let open = header_button("review-open-github", "View on GitHub", cx)
            .ghd_tooltip("View on GitHub")
            .child(octicon(Octicon::LinkExternal, t.text))
            .on_click(move |_, _, cx| Dispatcher::open_pull_request_review_on_github(id, cx))
            .into_any_element();
        let close = header_button("review-close", "Close the review", cx)
            .ghd_tooltip("Close")
            .child(octicon(Octicon::X, t.text))
            .on_click(move |_, _, cx| Dispatcher::close_pull_request_review(id, cx))
            .into_any_element();
        header(
            if review.draft {
                Octicon::GitPullRequestDraft
            } else {
                Octicon::GitPullRequest
            },
            "Pull Request",
            Some(format!("#{} {}", review.number, review.title)),
            vec![refresh, open, close],
            cx,
        )
    }

    /// The pending review's count and Review changes….
    fn footer(&self, id: u64, review: &ListSnapshot, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let pending = review.pending;
        let unresolved = review.unresolved;
        let mut text = String::new();
        if pending > 0 {
            text.push_str(&format!(
                "{pending} pending comment{}",
                if pending == 1 { "" } else { "s" }
            ));
        }
        if unresolved > 0 {
            if !text.is_empty() {
                text.push_str(" • ");
            }
            text.push_str(&format!("{unresolved} unresolved",));
        }
        let disabled = review.signed_out || review.busy.is_some();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
            .p(SPACING())
            .border_t_1()
            .border_color(t.box_border)
            .bg(t.box_alt_background)
            .when(!text.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .truncate()
                        .child(text),
                )
            })
            .when_some(review.busy.clone(), |d, busy| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .truncate()
                        .child(busy),
                )
            })
            .when_some(
                review.error.clone().filter(|_| !review.composer_open),
                |d, error| d.child(crate::widgets::input_error(error, cx)),
            )
            .child(
                primary_button(
                    "review-changes",
                    if pending > 0 {
                        format!("Review changes… ({pending})")
                    } else {
                        "Review changes…".to_string()
                    },
                    disabled,
                    cx,
                )
                .w_full()
                .justify_center()
                .when(!disabled, |b| {
                    b.on_click(move |_, _, cx| Dispatcher::show_submit_pull_request_review(id, cx))
                }),
            )
    }
}

impl Render for PullRequestReviewList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = {
            let s = self.state.read(cx);
            s.selected.and_then(|id| {
                let review = crate::review_threads::review_state_of(s, id)?;
                Some((id, ListSnapshot::of(review)))
            })
        };
        let Some((id, review)) = loaded else {
            self.focused = false;
            return div().into_any_element();
        };
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let t = cx.ghd().clone();
        let header = self.header(id, &review, cx);
        let footer = self.footer(id, &review, cx);
        let list_focused = self.focus.contains_focused(window, cx);
        let counts = review.counts.clone();
        let files = review.files.clone();
        let selection = review.selection.clone();
        let focus = self.focus.clone();
        let body: AnyElement = if review.preparing && files.is_empty() {
            github_list::loading("the pull request", "review-spin", &t).into_any_element()
        } else if let Some(message) = review.prepare_error.clone().filter(|_| files.is_empty()) {
            github_list::error(
                message,
                move |_, cx| Dispatcher::review_current_pull_request(id, cx),
                cx,
            )
            .into_any_element()
        } else {
            let count = files.len() + 1;
            uniform_list("review-rows", count, move |range, _window, cx| {
                range
                    .map(|ix| {
                        let t = cx.ghd();
                        let is_selected = if ix == 0 {
                            selection == ReviewSelection::Overview
                        } else {
                            matches!(&selection, ReviewSelection::File(p) if *p == files[ix - 1].path)
                        };
                        let (bg, text, secondary) = if is_selected && list_focused {
                            (
                                t.box_selected_active_background,
                                t.box_selected_active_text,
                                t.box_selected_active_text,
                            )
                        } else if is_selected {
                            (
                                t.box_selected_background,
                                t.box_selected_text,
                                t.box_selected_text,
                            )
                        } else {
                            (t.background, t.text, t.text_secondary)
                        };
                        let hover_bg = t.list_item_hover_background;
                        let focus = focus.clone();
                        if ix == 0 {
                            return div()
                                .id("review-overview-row")
                                .a11y_row("Overview".to_string(), is_selected)
                                .h(row_height())
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(SPACING_HALF())
                                .px(SPACING())
                                .bg(bg)
                                .text_color(text)
                                .border_b_1()
                                .border_color(t.box_border)
                                .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                                .on_click(move |_, window, cx| {
                                    window.focus(&focus, cx);
                                    Dispatcher::select_review_overview(id, cx);
                                })
                                .child(octicon(Octicon::Comment, secondary).flex_none())
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Overview"),
                                )
                                .into_any_element();
                        }
                        let file: &CommittedFileChange = &files[ix - 1];
                        let (icon, color) = crate::diff_view::status_icon(file.status.kind, t);
                        let color = if is_selected && list_focused { text } else { color };
                        let path = file.path.clone();
                        let counts = counts.get(&file.path).copied().unwrap_or_default();
                        div()
                            .id(SharedString::from(format!("review-file-{}", file.path)))
                            .a11y_row(
                                format!("{}, {}", file.path, crate::widgets::status_label(&file.status)),
                                is_selected,
                            )
                            .ghd_tooltip(file.path.clone())
                            .h(row_height())
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .px(SPACING())
                            .bg(bg)
                            .text_color(text)
                            .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                            .on_click(move |_, window, cx| {
                                window.focus(&focus, cx);
                                Dispatcher::select_review_file(id, path.clone(), cx);
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(FONT_SIZE())
                                    .child(
                                        crate::path_label::path_label_element(
                                            crate::path_label::path_label(
                                                &file.path,
                                                file.status.kind,
                                                file.old_path.as_deref(),
                                            ),
                                            Vec::new(),
                                            secondary,
                                            text,
                                        )
                                        .truncate(),
                                    ),
                            )
                            .children(thread_badge(
                                counts.unresolved,
                                counts.total,
                                counts.pending,
                                cx,
                            ))
                            .child(octicon(icon, color).flex_none())
                            .into_any_element()
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
        div()
            .id("pull-request-review-list")
            .track_focus(&self.focus)
            .on_key_down(move |ev, _, cx| {
                let down = match ev.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        Dispatcher::close_pull_request_review(id, cx);
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
            .child(footer)
            .into_any_element()
    }
}
