//! Corvene addition (`797-stash-list`): the Stashes section at the bottom of
//! the changes list, one row per stash entry (command-line ones included),
//! in place of GHD's single Stashed Changes row
//! (`changes/changes-list.tsx` `renderStashedChanges`).

use std::time::{Duration, SystemTime};

use corvene_core::{Dispatcher, RepositoryState, StashEntry};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{MenuItem, mac_or};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative as relative_time;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;

/// Rows shown before the section scrolls.
const VISIBLE_ROWS: f32 = 5.;

/// The section: a "Stashes" header with the count that collapses it, then
/// the entries. `None` without any stash.
pub fn stash_list(
    id: u64,
    rs: &RepositoryState,
    collapsed: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Option<AnyElement> {
    if rs.stashes.is_empty() {
        return None;
    }
    let t = cx.ghd();
    let shown = rs
        .showing_stash
        .then(|| rs.shown_stash().map(|s| s.sha.clone()))
        .flatten();
    let can_restore = rs.info.as_ref().and_then(|i| i.current_branch()).is_some()
        && rs.conflict_state.is_none()
        && !rs.committing;
    // `1313-changelists`: stashes made from lists are named after them
    let lists = corvene_core::changelists::of(corvene_core::AppState::global(cx).read(cx), id);
    let header = div()
        .id("stash-list-header")
        .flex_none()
        .w_full()
        .min_h(ROW_HEIGHT())
        .px(SPACING())
        .flex()
        .flex_row()
        .items_center()
        .border_t_1()
        .border_color(t.box_border)
        .bg(t.secondary_button_background)
        .text_color(t.secondary_button_text)
        .text_size(FONT_SIZE())
        .cursor_pointer()
        .hover(move |s| s.bg(t.box_selected_background))
        .on_click(on_toggle)
        .child(octicon(Octicon::Stash, t.color_modified))
        .child(
            div()
                .flex_1()
                .mx(SPACING_HALF())
                .truncate()
                .child(format!("Stashes ({})", rs.stashes.len())),
        )
        .child(octicon(
            if collapsed {
                Octicon::ChevronRight
            } else {
                Octicon::ChevronDown
            },
            t.secondary_button_text,
        ));
    let rows = (!collapsed).then(|| {
        div()
            .id("stash-list-rows")
            .flex_none()
            .w_full()
            .max_h(ROW_HEIGHT() * VISIBLE_ROWS)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(rs.stashes.iter().map(|entry| {
                stash_row(
                    id,
                    entry,
                    shown.as_ref() == Some(&entry.sha),
                    can_restore,
                    lists
                        .as_deref()
                        .and_then(|l| corvene_core::changelists::stash_label(l, &entry.sha)),
                    cx,
                )
            }))
            .with_scrollbar()
    });
    Some(
        div()
            .flex_none()
            .w_full()
            .flex()
            .flex_col()
            .child(header)
            .children(rows)
            .into_any_element(),
    )
}

/// "main · 3 days ago".
fn stash_detail(entry: &StashEntry) -> String {
    let mut parts = Vec::new();
    if let Some(branch) = corvene_core::stash_list::stash_branch(entry) {
        parts.push(branch);
    }
    if let Ok(secs) = u64::try_from(entry.date)
        && secs > 0
    {
        parts.push(relative_time(
            SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
        ));
    }
    parts.join(" · ")
}

fn stash_row(
    id: u64,
    entry: &StashEntry,
    selected: bool,
    can_restore: bool,
    // `1313-changelists`: "Stash of <list>" for a stash made from a list
    list_label: Option<String>,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let from_list = list_label.is_some();
    let title = list_label.unwrap_or_else(|| corvene_core::stash_list::stash_title(entry));
    let detail = stash_detail(entry);
    let desktop = entry.branch.is_some();
    let (text, muted) = if selected {
        (t.box_selected_active_text, t.box_selected_active_text)
    } else {
        (t.text, t.text_secondary)
    };
    let hover_bg = t.list_item_hover_background;
    let sha = entry.sha.clone();
    let menu_sha = entry.sha.clone();
    div()
        .id(SharedString::from(format!("stash-row-{}", entry.sha)))
        .a11y_row(
            if desktop {
                format!("{title}, made by Desktop, {detail}")
            } else {
                format!("{title}, {detail}")
            },
            selected,
        )
        .flex_none()
        .w_full()
        .h(ROW_HEIGHT())
        .px(SPACING())
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .text_size(FONT_SIZE())
        .cursor_pointer()
        .when(selected, |d| d.bg(t.box_selected_active_background))
        .when(!selected, move |d| d.hover(move |s| s.bg(hover_bg)))
        .on_click(move |_, _, cx| Dispatcher::view_stash(id, sha.clone(), cx))
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let items = stash_menu(id, menu_sha.clone(), can_restore);
                crate::native_menu::show_context_menu(items, ev.position, window, cx);
            },
        )
        // `1313-changelists`: made from a changelist
        .when(from_list, |d| {
            d.child(octicon(Octicon::ListUnordered, muted))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(text)
                .child(title),
        )
        // the stash Desktop keeps for a branch (switching branches restores it)
        .when(desktop, |d| d.child(octicon(Octicon::DeviceDesktop, muted)))
        .child(
            div()
                .flex_none()
                .max_w(relative(0.5))
                .truncate()
                .text_size(FONT_SIZE_SM())
                .text_color(muted)
                .child(detail),
        )
}

/// A stash row's menu.
fn stash_menu(id: u64, sha: String, can_restore: bool) -> Vec<MenuItem> {
    let (restore, apply, branch, discard) = (sha.clone(), sha.clone(), sha.clone(), sha);
    vec![
        MenuItem::new(mac_or("Restore Stash", "Restore stash"), move |_, cx| {
            Dispatcher::restore_stash_entry(id, restore.clone(), cx)
        })
        .enabled(can_restore),
        MenuItem::new(mac_or("Apply Stash", "Apply stash"), move |_, cx| {
            Dispatcher::apply_stash_entry(id, apply.clone(), cx)
        })
        .enabled(can_restore),
        MenuItem::new(
            mac_or("Create Branch from Stash…", "Create branch from stash…"),
            move |_, cx| Dispatcher::request_create_branch_from_stash(id, branch.clone(), cx),
        )
        .enabled(can_restore),
        MenuItem::separator(),
        MenuItem::new(
            mac_or("Discard Stash…", "Discard stash…"),
            move |_, cx| Dispatcher::request_discard_stash_entry(id, discard.clone(), cx),
        ),
    ]
}
