//! Request Reviewers: a Corvene extra with no GHD counterpart (flag
//! `336-request-reviewers`). Lists the repository's collaborators (narrowed
//! by a filter box, the pull request's author left out) with a checkbox
//! each; the users already asked for a review start ticked. Applying asks
//! the newly ticked ones and withdraws the request from the unticked ones.

use std::collections::BTreeSet;

use corvene_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon, spin};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::checkbox;

pub struct RequestReviewersDialog {
    state: Entity<AppState>,
    repo: u64,
    number: u64,
    filter: Entity<InputState>,
    /// Who was asked when the dialog opened (logins).
    requested: BTreeSet<String>,
    ticked: BTreeSet<String>,
}

impl RequestReviewersDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        number: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let handle = filter.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Dispatcher::load_collaborators(repo, cx);
        let requested: BTreeSet<String> = state
            .read(cx)
            .pull_requests_for(repo)
            .iter()
            .find(|pr| pr.number == number)
            .map(|pr| pr.requested_reviewers.clone())
            .unwrap_or_default()
            .into_iter()
            .collect();
        Self {
            state,
            repo,
            number,
            filter,
            ticked: requested.clone(),
            requested,
        }
    }

    fn toggle(&mut self, login: String, cx: &mut Context<Self>) {
        if !self.ticked.remove(&login) {
            self.ticked.insert(login);
        }
        cx.notify();
    }
}

impl Render for RequestReviewersDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let query = self.filter.read(cx).value().trim().to_string();
        let (collaborators, loading, title, author) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let pr = s
                .pull_requests_for(self.repo)
                .iter()
                .find(|pr| pr.number == self.number);
            (
                rs.and_then(|r| r.collaborators.clone()),
                rs.is_some_and(|r| r.collaborators_loading),
                pr.map(|pr| pr.title.clone()).unwrap_or_default(),
                pr.map(|pr| pr.author.clone()).unwrap_or_default(),
            )
        };
        let shown: Vec<String> = collaborators
            .as_deref()
            .map(|all| {
                all.iter()
                    .filter(|l| !l.eq_ignore_ascii_case(&author))
                    .filter(|l| {
                        query.is_empty() || corvene_core::filter::fuzzy_match(&query, l).is_some()
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let weak = cx.weak_entity();
        let hover_bg = t.list_item_hover_background;
        let list = div()
            .id("request-reviewers-list")
            .h(zpx(260.))
            .overflow_y_scroll()
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_col()
            .when(loading, |d| {
                d.child(
                    div()
                        .p(SPACING())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .text_color(t.text_secondary)
                        .child(spin(
                            octicon(Octicon::SyncClockwise, t.text_secondary),
                            "request-reviewers-spin",
                        ))
                        .child("Loading collaborators…"),
                )
            })
            .when(!loading && shown.is_empty(), |d| {
                d.child(
                    div()
                        .p(SPACING())
                        .text_color(t.text_secondary)
                        .child("Sorry, I can't find anyone to ask"),
                )
            })
            .children(shown.into_iter().enumerate().map(|(ix, login)| {
                let weak = weak.clone();
                let ticked = self.ticked.contains(&login);
                let toggled = login.clone();
                div()
                    .id(("request-reviewers-row", ix))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .px(SPACING())
                    .py(SPACING_HALF())
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(move |_, _, cx| {
                        weak.update(cx, |this, cx| this.toggle(toggled.clone(), cx))
                            .ok();
                    })
                    .child(checkbox(("request-reviewers-check", ix), ticked, false, cx))
                    .child(octicon(Octicon::Person, t.text_secondary).flex_none())
                    .child(div().flex_1().min_w_0().truncate().child(login))
            }))
            .with_scrollbar();
        let content = div()
            .w(crate::theme::fit_width(400.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(zpx(4.))
                    .child(format!("#{}", self.number))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    ),
            )
            .child(crate::widgets::filter_text_box(
                "request-reviewers-filter",
                &self.filter,
                Some(octicon(Octicon::Search, t.text_secondary)),
                window,
                cx,
            ))
            .child(list);
        let add: Vec<String> = self.ticked.difference(&self.requested).cloned().collect();
        let remove: Vec<String> = self.requested.difference(&self.ticked).cloned().collect();
        let unchanged = add.is_empty() && remove.is_empty();
        let (repo, number) = (self.repo, self.number);
        dialog(
            "dialog-request-reviewers",
            mac_or("Request Reviewers", "Request reviewers"),
            content,
            vec![
                DialogButton {
                    id: "request-reviewers-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "request-reviewers-ok",
                    label: mac_or("Request Reviewers", "Request reviewers").into(),
                    primary: true,
                    disabled: unchanged,
                    on_click: Box::new(move |_, cx| {
                        if unchanged {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::request_reviewers(
                            repo,
                            number,
                            add.clone(),
                            remove.clone(),
                            cx,
                        );
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
