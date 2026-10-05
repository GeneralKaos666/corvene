//! Right-click menus. GHD shows native NSMenus on macOS (`showContextualMenu`);
//! GPUI has no native context-menu API, so this reproduces the system look:
//! 5 px inset, 22 px items, 13 px text, accent highlight, 8 px radius.
//!
//! Shift+F10 (and the Menu key off macOS) open the selected row's menu in
//! the repository, branch, changes, history and commit file lists
//! ([`RowMenuAnchor`]), as in GHD. Deviation (`621-context-menu-buttons`): a
//! "…" button at the end of those rows opens it too ([`row_menu_button`]);
//! GHD's menus open only by right-click (desktop/desktop#2718).

use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{CloseFoldout, OpenRowContextMenu};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::zpx;

pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// GHD's `__DARWIN__`: menu labels are Title Case on macOS and Sentence
/// case elsewhere.
pub const IS_MAC: bool = cfg!(target_os = "macos");

/// GHD `__DARWIN__ ? mac : other`.
pub const fn mac_or(mac: &'static str, other: &'static str) -> &'static str {
    if IS_MAC { mac } else { other }
}

/// Shared labels, as exported by GHD `app/src/ui/lib/context-menu.ts`.
pub mod labels {
    use super::mac_or;

    /// `CopyFilePathLabel`
    pub const COPY_FILE_PATH: &str = mac_or("Copy File Path", "Copy file path");
    /// `CopyRelativeFilePathLabel`
    pub const COPY_RELATIVE_FILE_PATH: &str =
        mac_or("Copy Relative File Path", "Copy relative file path");
    /// `CopySelectedPathsLabel`
    pub const COPY_SELECTED_PATHS: &str = mac_or("Copy Paths", "Copy paths");
    /// `CopySelectedRelativePathsLabel`
    pub const COPY_SELECTED_RELATIVE_PATHS: &str =
        mac_or("Copy Relative Paths", "Copy relative paths");
    /// `DefaultEditorLabel`
    pub const DEFAULT_EDITOR: &str = mac_or("Open in External Editor", "Open in external editor");
    /// `DefaultShellLabel`
    pub const DEFAULT_SHELL: &str = mac_or("Open in Shell", "Open in shell");
    /// `RevealInFileManagerLabel`
    pub const REVEAL_IN_FILE_MANAGER: &str = if cfg!(target_os = "macos") {
        "Reveal in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show in your File Manager"
    };
    /// `OpenWithDefaultProgramLabel`
    pub const OPEN_WITH_DEFAULT_PROGRAM: &str =
        mac_or("Open with Default Program", "Open with default program");

    /// `Open in ${externalEditorLabel}` (the same pattern on every platform;
    /// the editor name is lower case only for the generic fallback).
    pub fn open_in(app: &str) -> String {
        format!("Open in {app}")
    }
}

#[derive(Clone)]
pub enum MenuItemKind {
    Action(MenuAction),
    Submenu(Vec<MenuItem>),
    Separator,
}

#[derive(Clone)]
pub struct MenuItem {
    pub label: SharedString,
    pub enabled: bool,
    pub kind: MenuItemKind,
    /// `type: 'checkbox'` items show a check mark column.
    pub checked: Option<bool>,
    /// A 16 px picture in front of the label (the editor and shell menus'
    /// application icons); the menus drawn by `views_menu` and macOS's
    /// `NSMenu`s show it.
    pub icon: Option<std::sync::Arc<Image>>,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            kind: MenuItemKind::Action(Rc::new(action)),
            checked: None,
            icon: None,
        }
    }

    pub fn checkbox(
        label: impl Into<SharedString>,
        checked: bool,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            checked: Some(checked),
            ..Self::new(label, action)
        }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            kind: MenuItemKind::Submenu(items),
            checked: None,
            icon: None,
        }
    }

    pub fn separator() -> Self {
        Self {
            label: SharedString::default(),
            enabled: false,
            kind: MenuItemKind::Separator,
            checked: None,
            icon: None,
        }
    }

    pub fn icon(mut self, icon: Option<std::sync::Arc<Image>>) -> Self {
        self.icon = icon;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

const ITEM_HEIGHT: f32 = 22.;
/// Native menus always reserve a check-mark column left of the labels.
const CHECK_COLUMN: f32 = 14.;
const SEPARATOR_HEIGHT: f32 = 11.;
const INSET: f32 = 5.;
const TEXT_SIZE: f32 = 13.;

pub struct ContextMenu {
    position: Point<Pixels>,
    items: Vec<MenuItem>,
    open_submenu: Option<usize>,
    focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl EventEmitter<DismissEvent> for ContextMenu {}

impl ContextMenu {
    /// Takes focus so Escape closes the menu; focus goes back where it was on dismiss.
    pub fn new(
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let previous_focus = window.focused(cx);
        window.focus(&focus_handle, cx);
        Self {
            position,
            items,
            open_submenu: None,
            focus_handle,
            previous_focus,
        }
    }

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev) = self.previous_focus.take() {
            window.focus(&prev, cx);
        }
        cx.emit(DismissEvent);
    }

    fn width(items: &[MenuItem]) -> Pixels {
        let longest = items
            .iter()
            .map(|i| i.label.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let check_column = if items.iter().any(|i| i.checked.is_some()) {
            16.
        } else {
            0.
        };
        zpx((longest * 6.8 + 48. + check_column).clamp(160., 440.))
    }

    fn height(items: &[MenuItem]) -> Pixels {
        let inner: f32 = items
            .iter()
            .map(|i| {
                if matches!(i.kind, MenuItemKind::Separator) {
                    SEPARATOR_HEIGHT
                } else {
                    ITEM_HEIGHT
                }
            })
            .sum();
        zpx(inner + INSET * 2. + 2.)
    }

    fn panel(&self, items: &[MenuItem], root: bool, cx: &Context<Self>) -> Div {
        let t = cx.ghd();
        let width = Self::width(items);
        div()
            .w(width)
            .p(zpx(INSET))
            .flex()
            .flex_col()
            .rounded(zpx(10.))
            .bg(t.menu_background)
            .border_1()
            .border_color(t.menu_border)
            .shadow(vec![BoxShadow {
                color: t.shadow,
                offset: point(zpx(0.), zpx(8.)),
                blur_radius: crate::theme::sizes::css_blur(24.),
                spread_radius: zpx(0.),
                inset: false,
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .children(
                items
                    .iter()
                    .enumerate()
                    .map(move |(idx, item)| self.item(idx, item, root, width, cx)),
            )
    }

    fn item(
        &self,
        idx: usize,
        item: &MenuItem,
        root: bool,
        width: Pixels,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        if matches!(item.kind, MenuItemKind::Separator) {
            return div()
                .h(zpx(1.))
                .my(zpx(5.))
                .mx(zpx(10.))
                .flex_none()
                .bg(t.menu_border)
                .into_any_element();
        }
        let enabled = item.enabled;
        let is_submenu = matches!(item.kind, MenuItemKind::Submenu(_));
        let open = root && self.open_submenu == Some(idx);
        let accent_bg = t.menu_highlight;
        let accent_text = t.menu_highlight_text;
        let id = if root {
            ("ctx-item", idx)
        } else {
            ("ctx-subitem", idx)
        };
        let mut el = div()
            .id(id)
            .relative()
            .h(zpx(ITEM_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .pl(zpx(6.))
            .pr(zpx(12.))
            .rounded(zpx(5.))
            .text_size(zpx(TEXT_SIZE))
            .text_color(if enabled {
                t.menu_text
            } else {
                t.menu_text_disabled
            })
            .when(open, |d| d.bg(accent_bg).text_color(accent_text))
            .when(enabled, |d| {
                d.hover(move |s| s.bg(accent_bg).text_color(accent_text))
            })
            .child(
                div()
                    .w(zpx(CHECK_COLUMN))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if item.checked == Some(true) {
                        "✓"
                    } else {
                        ""
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(item.label.clone()),
            );
        if is_submenu {
            el = el.child(octicon(Octicon::ChevronRight, t.text_secondary).size(zpx(12.)));
        }
        if root && enabled {
            el = el.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    let next = if is_submenu { Some(idx) } else { None };
                    if this.open_submenu != next {
                        this.open_submenu = next;
                        cx.notify();
                    }
                }
            }));
        }
        match &item.kind {
            MenuItemKind::Action(action) if enabled => {
                let action = action.clone();
                el = el.on_click(cx.listener(move |this, _, window, cx| {
                    this.dismiss(window, cx);
                    action(window, cx);
                }));
            }
            MenuItemKind::Submenu(children) if open => {
                el = el.child(
                    div()
                        .absolute()
                        .left(width - zpx(16.))
                        .top(zpx(-INSET - 1.))
                        .child(self.panel(children, false, cx)),
                );
            }
            _ => {}
        }
        el.into_any_element()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let width = Self::width(&self.items);
        let height = Self::height(&self.items);
        let mut x = self.position.x;
        let mut y = self.position.y;
        if x + width > viewport.width {
            x = viewport.width - width - zpx(4.);
        }
        if y + height > viewport.height {
            // native menus open upward when there is no room below the pointer
            y = self.position.y - height;
        }
        if x < zpx(0.) {
            x = zpx(0.);
        }
        if y < zpx(0.) {
            y = zpx(0.);
        }
        let panel = self.panel(&self.items, true, cx);
        deferred(
            anchored().position(point(zpx(0.), zpx(0.))).child(
                div()
                    .id("context-menu-overlay")
                    .track_focus(&self.focus_handle)
                    .relative()
                    .w(viewport.width)
                    .h(viewport.height)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| this.dismiss(window, cx)),
                    )
                    // Escape is bound to CloseFoldout; bindings win over key
                    // listeners, so handle the action (and raw key as fallback).
                    .on_action(
                        cx.listener(|this, _: &CloseFoldout, window, cx| this.dismiss(window, cx)),
                    )
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        if ev.keystroke.key == "escape" {
                            this.dismiss(window, cx);
                        }
                    }))
                    .child(div().absolute().left(x).top(y).child(panel)),
            ),
        )
        .with_priority(30)
    }
}

// ---- Shift+F10 / the Menu key, and the rows' "…" button ----

/// A list's selected row for the keyboard's context-menu keys (GHD
/// `app.tsx` `onMacOSWindowKeyDown`: Shift+F10 sends `contextmenu` to the
/// focused row; Chromium does the same for Shift+F10 and the Menu key on
/// Windows and Linux): where the row was last painted and the list's scroll
/// offset then, so a row scrolled since is found where it is now.
/// Where a row was painted and the list's scroll offset then.
type PaintedRow = Option<(Bounds<Pixels>, Point<Pixels>)>;

#[derive(Clone)]
pub struct RowMenuAnchor {
    painted: Rc<std::cell::Cell<PaintedRow>>,
    scroll: ScrollHandle,
}

impl RowMenuAnchor {
    pub fn new(scroll: &ScrollHandle) -> Self {
        Self {
            painted: Rc::default(),
            scroll: scroll.clone(),
        }
    }

    pub fn for_uniform_list(scroll: &UniformListScrollHandle) -> Self {
        Self::new(&scroll.0.borrow().base_handle)
    }

    /// The invisible child the selected row carries.
    pub fn track(&self) -> impl IntoElement + use<> {
        let painted = self.painted.clone();
        let scroll = self.scroll.clone();
        canvas(
            move |bounds, _, _| painted.set(Some((bounds, scroll.offset()))),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// Shift+F10 / Menu: the selected row is right-clicked where it is now,
    /// so its own menu opens there (nothing while it is scrolled out of
    /// view).
    pub fn open(&self, window: &mut Window, cx: &mut App) {
        let Some((bounds, offset)) = self.painted.get() else {
            return;
        };
        let moved = self.scroll.offset() - offset;
        let at = point(
            bounds.origin.x + moved.x + (bounds.size.width / 2.).min(zpx(40.)),
            bounds.origin.y + moved.y + bounds.size.height / 2.,
        );
        if self.scroll.bounds().contains(&at) {
            right_click_at(at, window, cx);
        }
    }

    /// The list element's handler for [`OpenRowContextMenu`].
    pub fn action_handler(&self) -> impl Fn(&OpenRowContextMenu, &mut Window, &mut App) + use<> {
        let anchor = self.clone();
        move |_, window, cx| anchor.open(window, cx)
    }
}

/// A right-click at `position`, after the event being handled: the row
/// under it opens its menu as for the mouse.
fn right_click_at(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.defer(cx, move |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Right,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Right,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
            cx,
        );
    });
}
/// Corvene (`621-context-menu-buttons`): whether rows show the "…" button.
pub fn row_menu_buttons(cx: &App) -> bool {
    corvene_core::AppState::global(cx)
        .read(cx)
        .flags
        .bool(corvene_core::flags::ids::CONTEXT_MENU_BUTTONS)
}

/// Corvene (`621-context-menu-buttons`): a "…" button for the end of a list
/// row whose element is `.group(group)`: shown while the row is hovered or
/// `selected`, it opens the row's context menu at the button (a right-click
/// there, which reaches the row's own handler).
pub fn row_menu_button(
    id: impl Into<ElementId>,
    group: &'static str,
    selected: bool,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(zpx(20.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(zpx(4.))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(mac_or("Show Context Menu", "Show context menu"))
        .when(!selected, |d| {
            d.invisible().group_hover(group, |s| s.visible())
        })
        .hover(|s| s.bg(color.opacity(0.15)))
        .child(octicon(Octicon::KebabHorizontal, color).size(zpx(16.)))
        .on_mouse_down(MouseButton::Left, |ev: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            right_click_at(ev.position, window, cx);
        })
}
