//! GitHub Desktop's Windows title bar (`ui/window/title-bar.tsx`,
//! `window-controls.tsx`, `_title-bar.scss`, `app.tsx#renderTitlebar`): the
//! window has no frame of its own; a 28 px strip in
//! `--win32-title-bar-background-color` holds the app icon, the app menu bar
//! (`crate::menu_bar`) and replicas of the Windows 10 window controls, and
//! is where the window is dragged by.
//!
//! During the welcome flow the bar is the `light-title-bar`: transparent,
//! over the page (`position: fixed`), without icon or menu. In full screen
//! there is no bar at all unless the app menu is in use.
//!
//! The controls and the drag area are declared to Windows as what they are
//! (`WindowControlArea`), so Windows does the minimising, maximising,
//! closing, dragging, double-click and snap layouts itself.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::prelude::*;
use gpui_kit::*;

/// `--win32-title-bar-height`, its 1 px bottom border included.
pub const HEIGHT: f32 = 28.;
/// The 16 px `.app-icon` with `margin: 0 var(--spacing)` either side: where
/// the menu bar's buttons start.
pub const MENU_BAR_LEFT: f32 = 36.;
/// `.window-controls button { width: 45px }`
const CONTROL_WIDTH: f32 = 45.;

/// `--win32-title-bar-background-color` (`$gray-900`)
const BACKGROUND: u32 = 0x24292e;

/// How the bar is shown (`app.tsx#renderTitlebar`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The dark strip above the page.
    Dark,
    /// The welcome flow's: transparent, over the page.
    Light,
    /// Full screen without the app menu in use: no bar.
    Hidden,
}

/// Whether the page starts at the top of the window (the bar lies over it,
/// or is not there).
static PAGE_AT_TOP: AtomicBool = AtomicBool::new(false);

/// The mode for this frame, decided by the window's root
/// (`menu_bar::MenuBarShell`) before the page is laid out.
pub fn set_mode(mode: Mode) {
    PAGE_AT_TOP.store(mode != Mode::Dark, Ordering::Relaxed);
}

/// Where the page starts in the window (`theme::page_top`).
pub fn page_top() -> f32 {
    if PAGE_AT_TOP.load(Ordering::Relaxed) {
        0.
    } else {
        HEIGHT
    }
}

/// One window control: an icon drawn to a 10 × 10 box, grey until hovered.
fn control(
    name: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    light: bool,
) -> impl IntoElement {
    let close = area == WindowControlArea::Close;
    // `.window-controls button`, and `.light-title-bar`'s
    let (hover, active, hover_icon) = match (close, light) {
        (true, false) => (0xe81123, 0xbf0f1d, 0xffffff),
        (true, true) => (0xe81123, 0xf1707a, 0xffffff),
        (false, false) => (0x888888, 0x666666, 0xffffff),
        (false, true) => (0xe5e5e5, 0xcccccc, 0x000000),
    };
    div()
        .id(name)
        .group(name)
        .w(px(CONTROL_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(|style| style.bg(rgb(hover)))
        .active(|style| style.bg(rgb(active)))
        .child(
            svg()
                .path(icon)
                .size(px(10.))
                .flex_none()
                .text_color(rgb(0xa0a0a0))
                .group_hover(name, |style| style.text_color(rgb(hover_icon))),
        )
}

/// The title bar around `menu_bar`, which is `menu_bar_width` wide.
pub fn title_bar(
    menu_bar: impl IntoElement,
    menu_bar_width: f32,
    mode: Mode,
    window: &Window,
) -> AnyElement {
    let light = mode == Mode::Light;
    let drag = || div().h_full().window_control_area(WindowControlArea::Drag);
    // the menu bar stays in the tree while hidden: it listens for Alt
    let menu_bar = div()
        .h_full()
        .flex_none()
        .overflow_hidden()
        .w(px(if mode == Mode::Dark {
            menu_bar_width
        } else {
            0.
        }))
        .child(menu_bar);
    if mode == Mode::Hidden {
        return div()
            .id("title-bar")
            .w_full()
            .h_0()
            .flex_none()
            .overflow_hidden()
            .child(menu_bar)
            .into_any_element();
    }
    div()
        .id("title-bar")
        .w_full()
        .h(px(HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .when(!light, |bar| {
            bar.bg(rgb(BACKGROUND))
                .relative()
                // `border-bottom: 1px solid #000`, under the menu bar so that
                // an open menu's button reaches its pane
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_b_1()
                        .border_color(rgb(0x000000)),
                )
                // `showAppIcon`: not in the welcome flow
                .child(
                    drag()
                        .w(px(MENU_BAR_LEFT))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        // `.app-icon`: a 16 px mark in
                        // `--toolbar-button-secondary-color`
                        .child(
                            svg()
                                .path("icon/Corvene-mark.svg")
                                .size(px(16.))
                                .flex_none()
                                .text_color(rgb(0xd1d5da)),
                        ),
                )
        })
        .child(menu_bar)
        .child(drag().flex_1())
        // no window controls in full screen
        .when(!window.is_fullscreen(), |bar| {
            let (maximize, icon) = if window.is_maximized() {
                ("window-restore", "controls/window-restore.svg")
            } else {
                ("window-maximize", "controls/window-maximize.svg")
            };
            bar.child(control(
                "window-minimize",
                "controls/window-minimize.svg",
                WindowControlArea::Min,
                light,
            ))
            .child(control(maximize, icon, WindowControlArea::Max, light))
            .child(control(
                "window-close",
                "controls/window-close.svg",
                WindowControlArea::Close,
                light,
            ))
        })
        .into_any_element()
}
