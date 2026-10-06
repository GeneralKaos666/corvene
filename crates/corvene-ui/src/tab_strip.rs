//! Repository tabs (Corvene `430-repository-tabs`; `.docs/multiple-windows.md`
//! §4). GitHub Desktop has no tabs: its toolbar's repository button switches
//! the one view. The strip sits where GHD's `#desktop-app-title-bar` is on
//! macOS (the traffic lights keep its left 78 px, as in Safari and Zed) and
//! above the toolbar elsewhere: one tab per `WorkspaceState::tabs` entry,
//! the selected one in the toolbar's colour so it joins the toolbar below,
//! a × on hover (middle click closes too), a + that opens the repository
//! list, a context menu with Close Tab, Close Other Tabs and, with
//! `429-multiple-windows`, Open in New Window.

use corvene_core::{AppState, Dispatcher, Foldout};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{MenuItem, mac_or};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::tooltip;

/// The strip's height: the macOS title bar's, where it is drawn.
pub fn tab_strip_height() -> Pixels {
    TITLE_BAR_HEIGHT()
}

/// The space the traffic lights take on macOS (hiddenInset, 3 × 12 px with
/// their 8 px gaps and the 9 px inset, rounded up to Safari's).
fn leading_inset() -> Pixels {
    if cfg!(target_os = "macos") {
        zpx(78.)
    } else {
        SPACING_HALF()
    }
}

/// The strip's content (the tabs and the + button); the caller draws the
/// bar it sits in.
pub fn tab_strip(state: &AppState, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let selected = state.selected;
    let multiple_windows = state.flags.bool(corvene_core::flags::ids::MULTIPLE_WINDOWS);
    let tabs: Vec<(u64, String, String)> = state
        .tabs
        .iter()
        .filter_map(|id| state.repository(*id))
        .map(|r| (r.id, r.name(), r.path.to_string_lossy().into_owned()))
        .collect();
    let count = tabs.len();
    div()
        .id("tab-strip")
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_row()
        .items_end()
        .pl(leading_inset())
        .gap(zpx(2.))
        .children(tabs.into_iter().map(|(id, name, path)| {
            let is_selected = selected == Some(id);
            let (bg, text) = if is_selected {
                (t.toolbar_background, t.toolbar_text)
            } else {
                (transparent_black(), t.toolbar_text_secondary)
            };
            let hover_bg = t.toolbar_button_hover_background;
            let close_hover = t.toolbar_button_active_background;
            div()
                .id(("tab", id as usize))
                .group("tab")
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .h(tab_strip_height() - zpx(6.))
                .min_w(zpx(60.))
                .max_w(zpx(220.))
                .pl(zpx(10.))
                .pr(zpx(6.))
                .gap(zpx(4.))
                .rounded_t(BORDER_RADIUS())
                .bg(bg)
                .text_color(text)
                .text_size(FONT_SIZE_SM())
                .when(!is_selected, |d| d.hover(|d| d.bg(hover_bg)))
                .cursor_default()
                .tooltip(tooltip(path))
                .on_click(move |event, window, cx| {
                    // the title bar's double click (zoom) must not reach here
                    cx.stop_propagation();
                    if event.click_count() == 1 {
                        Dispatcher::select_repository(id, cx);
                        let _ = window;
                    }
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                    cx.stop_propagation();
                    Dispatcher::close_tab(Some(id), cx);
                })
                .on_mouse_down(
                    MouseButton::Right,
                    move |ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        crate::native_menu::show_context_menu(
                            tab_menu_items(id, count, multiple_windows),
                            ev.position,
                            window,
                            cx,
                        );
                    },
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(name),
                )
                .child(
                    div()
                        .id(("tab-close", id as usize))
                        .flex_none()
                        .size(zpx(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(zpx(3.))
                        .when(!is_selected, |d| {
                            d.invisible().group_hover("tab", |d| d.visible())
                        })
                        .hover(|d| d.bg(close_hover))
                        .cursor_default()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            Dispatcher::close_tab(Some(id), cx);
                        })
                        .child(octicon(Octicon::X, text).size(zpx(12.))),
                )
        }))
        .child(
            div()
                .id("tab-add")
                .flex_none()
                .size(zpx(22.))
                .mb(zpx(2.))
                .ml(zpx(2.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(zpx(4.))
                .hover(|d| d.bg(t.toolbar_button_hover_background))
                .cursor_default()
                .tooltip(tooltip(mac_or("Open a Repository", "Open a repository")))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| {
                    cx.stop_propagation();
                    Dispatcher::toggle_foldout(Foldout::Repository, cx);
                })
                .child(octicon(Octicon::Plus, t.toolbar_text_secondary).size(zpx(14.))),
        )
}

/// A tab's context menu.
fn tab_menu_items(id: u64, count: usize, multiple_windows: bool) -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem::new(mac_or("Close Tab", "Close tab"), move |_, cx| {
            Dispatcher::close_tab(Some(id), cx);
        })
        .enabled(count > 1),
        MenuItem::new(
            mac_or("Close Other Tabs", "Close other tabs"),
            move |_, cx| {
                let others: Vec<u64> = corvene_core::AppState::global(cx)
                    .read(cx)
                    .tabs
                    .iter()
                    .copied()
                    .filter(|t| *t != id)
                    .collect();
                Dispatcher::select_repository(id, cx);
                for other in others {
                    Dispatcher::close_tab(Some(other), cx);
                }
            },
        )
        .enabled(count > 1),
    ];
    if multiple_windows {
        items.extend([
            MenuItem::separator(),
            MenuItem::new(
                mac_or("Open in New Window", "Open in new window"),
                move |_, cx| {
                    crate::windows::move_tab_to_new_window(id, cx);
                },
            )
            .enabled(count > 1),
        ]);
    }
    items
}
