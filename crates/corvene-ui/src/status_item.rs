//! The macOS menu bar status item (Corvene addition, flag
//! `428-menu-bar-status-item`): a real `NSStatusItem`, as the project
//! prefers AppKit for system chrome. Its icon is the worst state of the
//! watched repositories' checks (an SF Symbol template image), its title
//! the summed ahead/behind counts, and its menu lists each watched
//! repository (click: open it) with its branch's sync state and checks, a
//! failing check per line (click: its page on GitHub), then Open Corvene
//! and Watched Repositories…. GHD has no status item.
//!
//! [`sync`] is called from the app's state observer with the
//! [`MenuBarModel`] the state produces (`AppState::menu_bar_model`); the
//! item is created on the first model, rebuilt when the model changes and
//! removed when the model goes away. The menu is built by
//! [`crate::native_menu`]'s `NSMenu` builder with a target of its own: an
//! item's `statusAction:` runs on the main thread outside GPUI, so it is
//! dispatched back into GPUI through a stored `AsyncApp`.

// objc's macros probe a `cargo-clippy` cfg that this crate does not declare
#![allow(unexpected_cfgs)]
// cocoa 0.26 deprecates its bindings in favour of objc2; GPUI still ships them
#![allow(deprecated)]

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use cocoa::base::{YES, id, nil};
use cocoa::foundation::NSString;
use corvene_core::menu_bar_status::{MenuBarModel, OverallState};
use corvene_core::{Dispatcher, PreferencesTab};
use gpui_kit::*;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

use crate::context_menu::{MenuAction, MenuItem};

/// `NSVariableStatusItemLength`
const VARIABLE_LENGTH: f64 = -1.0;
/// `NSImageLeft`
const IMAGE_LEFT: u64 = 2;

thread_local! {
    /// The `NSStatusItem` (retained), once shown.
    static ITEM: Cell<usize> = const { Cell::new(0) };
    /// The model the item was last built from.
    static SHOWN: RefCell<Option<MenuBarModel>> = const { RefCell::new(None) };
    /// The menu's actions by item tag.
    static ACTIONS: RefCell<Vec<Option<MenuAction>>> = const { RefCell::new(Vec::new()) };
    /// How `statusAction:` gets back into GPUI.
    static ASYNC: RefCell<Option<AsyncApp>> = const { RefCell::new(None) };
}

/// `statusAction:`: run the chosen item's action inside GPUI, on the next
/// turn of the run loop (the menu's tracking loop is still unwinding).
extern "C" fn status_action(_this: &Object, _sel: Sel, item: id) {
    let tag: i64 = unsafe { msg_send![item, tag] };
    let action = ACTIONS.with(|a| a.borrow().get(tag as usize).cloned().flatten());
    let Some(action) = action else { return };
    let Some(app) = ASYNC.with(|a| a.borrow().clone()) else {
        return;
    };
    app.spawn(async move |cx: &mut AsyncApp| {
        cx.update(|cx| {
            // the focused window (`429-multiple-windows`), hidden or not
            if let Some(entry) = crate::windows::focused(cx) {
                entry
                    .window
                    .update(cx, |_, window, cx| action(window, cx))
                    .ok();
            }
        });
    })
    .detach();
}

/// One shared `CorveneStatusItemTarget` instance receiving every item's action.
fn target() -> id {
    static TARGET: OnceLock<usize> = OnceLock::new();
    *TARGET.get_or_init(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl =
            ClassDecl::new("CorveneStatusItemTarget", superclass).expect("class registered once");
        decl.add_method(
            sel!(statusAction:),
            status_action as extern "C" fn(&Object, Sel, id),
        );
        let class: &'static Class = decl.register();
        let instance: id = msg_send![class, new];
        instance as usize
    }) as id
}

unsafe fn ns_string(text: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(text) }
}

/// The SF Symbol for the worst state.
fn symbol_name(state: OverallState) -> &'static str {
    match state {
        OverallState::Failing => "xmark.circle",
        OverallState::Pending => "clock",
        OverallState::Passing => "checkmark.circle",
        OverallState::NoChecks => "arrow.triangle.branch",
    }
}

/// Bring the main window back and select `id`.
fn open_repository(id: u64) -> MenuAction {
    std::rc::Rc::new(move |window: &mut Window, cx: &mut App| {
        Dispatcher::select_repository(id, cx);
        crate::native_window::show_window(window, cx);
        cx.activate(true);
    })
}

/// The menu for `model`.
fn menu_items(model: &MenuBarModel) -> Vec<MenuItem> {
    let mut items = Vec::new();
    for repo in &model.repos {
        items.push(MenuItem::new(repo.name.clone(), {
            let open = open_repository(repo.id);
            move |window, cx| open(window, cx)
        }));
        items.push(MenuItem::new_info(format!("    {}", repo.sync_label())).enabled(false));
        match &repo.checks {
            Some(checks) => {
                let glyph = if checks.failed > 0 {
                    "✗"
                } else if checks.pending > 0 || checks.conclusion.is_none() {
                    "●"
                } else {
                    "✓"
                };
                items.push(
                    MenuItem::new_info(format!("    {glyph} {}", checks.label())).enabled(false),
                );
                for failing in &checks.failing {
                    let url = failing.url.clone();
                    let has_url = url.is_some();
                    items.push(
                        MenuItem::new(format!("        ✗ {}", failing.name), move |_, cx| {
                            if let Some(url) = &url {
                                Dispatcher::open_url(url, cx);
                            }
                        })
                        .enabled(has_url),
                    );
                }
            }
            None => items.push(MenuItem::new_info("    No checks").enabled(false)),
        }
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::new("Open Corvene", |window, cx| {
        crate::native_window::show_window(window, cx);
        cx.activate(true);
    }));
    items.push(MenuItem::new("Watched Repositories…", |window, cx| {
        crate::native_window::show_window(window, cx);
        cx.activate(true);
        Dispatcher::open_preferences(PreferencesTab::Advanced, cx);
    }));
    items
}

impl MenuItem {
    /// An item that does nothing (an information line, shown disabled).
    fn new_info(label: impl Into<SharedString>) -> Self {
        Self::new(label, |_, _| {})
    }
}

/// Show, update or remove the status item for `model`.
pub fn sync(model: Option<&MenuBarModel>, cx: &mut App) {
    let unchanged = SHOWN.with(|s| s.borrow().as_ref() == model);
    if unchanged {
        return;
    }
    SHOWN.with(|s| *s.borrow_mut() = model.cloned());
    ASYNC.with(|a| *a.borrow_mut() = Some(cx.to_async()));
    let Some(model) = model else {
        remove();
        return;
    };
    let items = menu_items(model);
    let mut actions: Vec<Option<MenuAction>> = Vec::new();
    unsafe {
        let item = ITEM.with(|i| i.get());
        let item: id = if item == 0 {
            let bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            let item: id = msg_send![bar, statusItemWithLength: VARIABLE_LENGTH];
            let _: () = msg_send![item, retain];
            ITEM.with(|i| i.set(item as usize));
            item
        } else {
            item as id
        };
        let button: id = msg_send![item, button];
        let symbol = ns_string(symbol_name(model.overall()));
        let description = ns_string(&model.description());
        let image: id = msg_send![class!(NSImage), imageWithSystemSymbolName: symbol accessibilityDescription: description];
        if image != nil {
            let _: () = msg_send![image, setTemplate: YES];
            let _: () = msg_send![button, setImage: image];
            let _: () = msg_send![button, setImagePosition: IMAGE_LEFT];
        }
        let _: () = msg_send![button, setTitle: ns_string(&model.title())];
        let _: () = msg_send![button, setToolTip: description];
        let menu =
            crate::native_menu::build_menu_for(&items, &mut actions, target(), sel!(statusAction:));
        let _: () = msg_send![item, setMenu: menu];
        let _: () = msg_send![menu, release];
    }
    ACTIONS.with(|a| *a.borrow_mut() = actions);
}

/// Whether the item is in the menu bar.
pub fn is_shown() -> bool {
    ITEM.with(|i| i.get() != 0)
}

/// Take the item out of the menu bar.
fn remove() {
    let item = ITEM.with(|i| i.replace(0));
    if item == 0 {
        return;
    }
    unsafe {
        let item = item as id;
        let bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
        let _: () = msg_send![bar, removeStatusItem: item];
        let _: () = msg_send![item, release];
    }
    ACTIONS.with(|a| a.borrow_mut().clear());
}
