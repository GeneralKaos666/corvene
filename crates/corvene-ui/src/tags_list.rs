//! Corvene `1219-tag-manager`: the History sidebar while Branch › Tags… is
//! open (`corvene_core::tag_manager`). GitHub Desktop has no tag list; tags
//! only show as labels on History's commits.
//!
//! A header counts the tags and offers Fetch All Tags (as Repository ›
//! Fetch All Tags, `899`) and Close; a filter box narrows the list by name,
//! annotation or commit summary. Each row shows the tag (marked Not pushed
//! while it is in `tagsToPush`), then its date, short SHA and the
//! annotation's message or the commit's summary. Selecting a row shows the
//! commit in the usual commit view; its context menu checks the commit out,
//! pushes the tag, deletes it here or on the remote, creates a branch or a
//! release there, or copies the name or SHA.

use std::time::{Duration, SystemTime};

use corvene_core::tag_manager::{TagsViewState, filter_tags};
use corvene_core::{AppState, Dispatcher, Popup};
use corvene_git::TagInfo;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{MenuItem, mac_or};
use crate::github_list::{self, header, header_button, pill};
use crate::history::commit_row_height;
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::GhdTooltip;

pub struct TagsList {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    /// The selected tag last scrolled into view.
    scrolled_to: Option<String>,
    /// Focused since it opened (arrows and Escape work at once).
    focused: bool,
}

impl TagsList {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle().tab_stop(true),
            scrolled_to: None,
            focused: false,
        }
    }

    /// The tags the filter lets through, as indexes into `view.tags`.
    fn shown(&self, view: &TagsViewState, cx: &App) -> Vec<usize> {
        filter_tags(&view.tags, &self.filter.read(cx).value())
    }

    /// ↑ / ↓ (also from the filter box).
    fn move_selection(&mut self, id: u64, down: bool, cx: &mut Context<Self>) {
        let next = {
            let s = self.state.read(cx);
            let Some(view) = s.repo_states.get(&id).and_then(|rs| rs.tags_view.as_ref()) else {
                return;
            };
            let shown = self.shown(view, cx);
            if shown.is_empty() {
                return;
            }
            let current = view
                .selected
                .as_deref()
                .and_then(|name| shown.iter().position(|&i| view.tags[i].name == name));
            let next = match (current, down) {
                (None, _) => 0,
                (Some(c), true) => (c + 1).min(shown.len() - 1),
                (Some(c), false) => c.saturating_sub(1),
            };
            view.tags[shown[next]].name.clone()
        };
        Dispatcher::select_tag(id, next, cx);
    }

    fn header(&self, id: u64, view: &TagsViewState, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let subtitle = view.loaded.then(|| {
            let n = view.tags.len();
            format!("{n} tag{}", if n == 1 { "" } else { "s" })
        });
        header(
            Octicon::Tag,
            "Tags",
            subtitle,
            vec![
                header_button("tags-fetch", "Fetch All Tags", cx)
                    .child(octicon(Octicon::Sync, t.text))
                    .on_click(move |_, _, cx| Dispatcher::fetch_all_tags(id, cx))
                    .into_any_element(),
                header_button("tags-close", "Close Tags", cx)
                    .child(octicon(Octicon::X, t.text))
                    .on_click(move |_, _, cx| Dispatcher::close_tags(id, cx))
                    .into_any_element(),
            ],
            cx,
        )
    }
}

/// What a row's context menu needs besides the tag.
#[derive(Clone)]
struct RowContext {
    remote: Option<String>,
    unpushed: bool,
    releases: bool,
}

fn row_menu(
    id: u64,
    tag: TagInfo,
    context: RowContext,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let RowContext {
        remote,
        unpushed,
        releases,
    } = context;
    let mut items = Vec::new();
    let sha = tag.sha.clone();
    items.push(MenuItem::new(
        mac_or("Check Out Tag…", "Check out tag…"),
        move |_, cx| Dispatcher::request_checkout_commit(id, sha.clone(), cx),
    ));
    let branch_sha = tag.sha.clone();
    items.push(MenuItem::new(
        mac_or("Create Branch from Tag…", "Create branch from tag…"),
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
    ));
    if releases {
        let (name, sha) = (tag.name.clone(), tag.sha.clone());
        items.push(MenuItem::new(
            mac_or("Create Release…", "Create release…"),
            move |_, cx| {
                Dispatcher::show_create_release(id, Some(name.clone()), Some(sha.clone()), cx)
            },
        ));
    }
    items.push(MenuItem::separator());
    let pushed = tag.name.clone();
    items.push(
        MenuItem::new(
            match &remote {
                Some(remote) => format!("Push Tag to {remote}"),
                None => mac_or("Push Tag", "Push tag").to_string(),
            },
            move |_, cx| Dispatcher::push_single_tag(id, pushed.clone(), cx),
        )
        .enabled(remote.is_some()),
    );
    items.push(MenuItem::separator());
    let copied = tag.name.clone();
    items.push(MenuItem::new(
        mac_or("Copy Tag Name", "Copy tag name"),
        move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(copied.clone())),
    ));
    let copied_sha = tag.sha.clone();
    items.push(MenuItem::new("Copy SHA", move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(copied_sha.clone()))
    }));
    items.push(MenuItem::separator());
    let deleted = tag.name.clone();
    items.push(MenuItem::new(
        mac_or("Delete Tag…", "Delete tag…"),
        move |_, cx| Dispatcher::request_delete_tag(id, deleted.clone(), cx),
    ));
    let deleted_remote = tag.name.clone();
    items.push(
        MenuItem::new(
            match &remote {
                Some(remote) => format!("Delete Tag from {remote}…"),
                None => mac_or("Delete Tag from Remote…", "Delete tag from remote…").to_string(),
            },
            move |_, cx| Dispatcher::request_delete_remote_tag(id, deleted_remote.clone(), cx),
        )
        // a tag created here and not pushed is not on the remote
        .enabled(remote.is_some() && !unpushed),
    );
    crate::native_menu::show_context_menu(items, position, window, cx);
}

/// A tag's date as History shows commit dates.
fn date_text(seconds: i64, now: SystemTime) -> String {
    let when = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds.max(0) as u64);
    if crate::format::prefer_absolute_dates() {
        crate::format::format_date(when)
    } else {
        crate::relative_time::relative_at(when, now)
    }
}

impl Render for TagsList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = {
            let s = self.state.read(cx);
            s.selected.and_then(|id| {
                let rs = s.repo_states.get(&id)?;
                let view = corvene_core::tag_manager::tags_of(s, rs)?.clone();
                let repo = s.repository(id)?;
                let context = RowContext {
                    remote: Dispatcher::current_remote_in(s, id).map(|r| r.name),
                    unpushed: false,
                    releases: s.flags.bool(corvene_core::flags::ids::RELEASES)
                        && repo.non_fork_github().is_some(),
                };
                Some((id, view, repo.tags_to_push.clone(), context))
            })
        };
        let Some((id, view, unpushed, context)) = loaded else {
            self.focused = false;
            return div().into_any_element();
        };
        if !self.focused {
            self.focused = true;
            self.scrolled_to = None;
            self.filter
                .update(cx, |input, cx| input.set_value("", window, cx));
            window.focus(&self.focus, cx);
        }
        let t = cx.ghd().clone();
        let header = self.header(id, &view, cx);
        let shown = self.shown(&view, cx);
        if view.selected != self.scrolled_to {
            self.scrolled_to = view.selected.clone();
            if let Some(row) = view
                .selected
                .as_deref()
                .and_then(|name| shown.iter().position(|&i| view.tags[i].name == name))
            {
                self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
            }
        }
        let list_focused = self.focus.contains_focused(window, cx);
        let body = if !view.loaded {
            github_list::loading("tags", "tags-spin", &t).into_any_element()
        } else if view.tags.is_empty() {
            github_list::message("No tags yet", &t).into_any_element()
        } else if shown.is_empty() {
            github_list::message("No tags match the filter", &t).into_any_element()
        } else {
            let row_height = commit_row_height(cx);
            let now = SystemTime::now();
            let focus = self.focus.clone();
            let tags = view.tags.clone();
            let selected = view.selected.clone();
            uniform_list("tag-rows", shown.len(), move |range, _window, _cx| {
                range
                    .map(|ix| {
                        let tag = &tags[shown[ix]];
                        let is_selected = selected.as_deref() == Some(tag.name.as_str());
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
                        let is_unpushed = unpushed.contains(&tag.name);
                        let context = RowContext {
                            unpushed: is_unpushed,
                            ..context.clone()
                        };
                        let detail = if tag.message.is_empty() {
                            tag.summary.clone()
                        } else {
                            tag.message.clone()
                        };
                        let mut byline = date_text(tag.seconds, now);
                        byline.push_str(" • ");
                        byline.push_str(&tag.sha[..tag.sha.len().min(7)]);
                        if !detail.is_empty() {
                            byline.push_str(" • ");
                            byline.push_str(&detail);
                        }
                        let name = tag.name.clone();
                        let menu_tag = tag.clone();
                        let (focus, menu_focus) = (focus.clone(), focus.clone());
                        let (click_name, menu_name) = (name.clone(), name.clone());
                        div()
                            .id(("tag-row", ix))
                            .ghd_tooltip(if tag.summary.is_empty() {
                                name.clone()
                            } else {
                                format!("{name}\n{}", tag.summary)
                            })
                            .h(row_height)
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .pl(SPACING())
                            .pr(SPACING() + SPACING_HALF())
                            .bg(bg)
                            .border_b_1()
                            .border_color(t.box_border)
                            .on_click(move |_, window, cx| {
                                window.focus(&focus, cx);
                                Dispatcher::select_tag(id, click_name.clone(), cx);
                            })
                            .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
                                cx.stop_propagation();
                                window.focus(&menu_focus, cx);
                                Dispatcher::select_tag(id, menu_name.clone(), cx);
                                row_menu(
                                    id,
                                    menu_tag.clone(),
                                    context.clone(),
                                    ev.position,
                                    window,
                                    cx,
                                );
                            })
                            .child(octicon(Octicon::Tag, secondary).flex_none())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(SPACING_HALF())
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(FONT_SIZE())
                                                    .line_height(zpx(18.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(text)
                                                    .child(name),
                                            )
                                            .when(is_unpushed, |d| {
                                                d.child(pill("Not pushed", secondary))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .mt(zpx(3.))
                                            .min_w_0()
                                            .truncate()
                                            .text_size(FONT_SIZE_SM())
                                            .line_height(zpx(16.5))
                                            .text_color(secondary)
                                            .child(byline),
                                    ),
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

        let filter_view = cx.entity().downgrade();
        let filter_box = div()
            .flex_none()
            .p(SPACING_HALF())
            .border_b_1()
            .border_color(t.box_border)
            .on_key_down(move |ev, _, cx| {
                let down = match ev.keystroke.key.as_str() {
                    "down" => true,
                    "up" => false,
                    _ => return,
                };
                cx.stop_propagation();
                filter_view
                    .update(cx, |this, cx| this.move_selection(id, down, cx))
                    .ok();
            })
            .child(crate::widgets::filter_text_box(
                "tags-filter",
                &self.filter,
                Some(octicon(Octicon::Search, t.text_secondary)),
                window,
                cx,
            ));

        let view_handle = cx.entity().downgrade();
        let t = cx.ghd();
        div()
            .id("tags-list")
            .track_focus(&self.focus)
            .on_key_down(move |ev, _, cx| {
                let down = match ev.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        Dispatcher::close_tags(id, cx);
                        return;
                    }
                    "down" => true,
                    "up" => false,
                    _ => return,
                };
                cx.stop_propagation();
                view_handle
                    .update(cx, |this, cx| this.move_selection(id, down, cx))
                    .ok();
            })
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(t.background)
            .child(header)
            .child(filter_box)
            .child(body)
            .into_any_element()
    }
}
