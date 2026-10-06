//! The open windows and the workspace each one shows (Corvene
//! `429-multiple-windows`; `.docs/multiple-windows.md`). GitHub Desktop has
//! one `BrowserWindow`; here a `Global` lists every window with its
//! `Workspace` view and `corvene_core::WorkspaceId`, so app-level code
//! (menu actions, URLs, the status item) finds the window it should act on
//! and a view's state observer runs for the window it is in.
//!
//! Opening a window is the binary's job (`main.rs` knows the window
//! options); it registers an opener with [`set_opener`] that
//! [`open_workspace_window`] calls.

use std::cell::RefCell;
use std::rc::Rc;

use corvene_core::{AppState, Dispatcher, WorkspaceId};
use gpui_kit::*;

use crate::workspace::Workspace;

/// One open window.
#[derive(Clone)]
pub struct WindowEntry {
    pub window: AnyWindowHandle,
    pub workspace: WorkspaceId,
    pub view: Entity<Workspace>,
}

type Opener = Rc<dyn Fn(WorkspaceId, &mut App)>;

#[derive(Default)]
struct Windows {
    /// In creation order; the first is the main window.
    entries: Vec<WindowEntry>,
    opener: Option<Opener>,
}

impl Global for Windows {}

fn registry(cx: &mut App) -> &mut Windows {
    cx.default_global::<Windows>()
}

/// A window opened: remember its workspace and view.
pub fn register(
    window: AnyWindowHandle,
    workspace: WorkspaceId,
    view: Entity<Workspace>,
    cx: &mut App,
) {
    let entries = &mut registry(cx).entries;
    entries.retain(|e| e.window != window);
    entries.push(WindowEntry {
        window,
        workspace,
        view,
    });
}

/// A window closed: forget it. Returns the workspace it showed.
pub fn unregister(window: WindowId, cx: &mut App) -> Option<WorkspaceId> {
    let entries = &mut registry(cx).entries;
    let index = entries
        .iter()
        .position(|e| e.window.window_id() == window)?;
    Some(entries.remove(index).workspace)
}

/// Every open window, the main one first.
pub fn entries(cx: &App) -> Vec<WindowEntry> {
    cx.try_global::<Windows>()
        .map(|w| w.entries.clone())
        .unwrap_or_default()
}

pub fn count(cx: &App) -> usize {
    cx.try_global::<Windows>().map_or(0, |w| w.entries.len())
}

/// The workspace `window` shows.
pub fn workspace_of(window: &Window, cx: &App) -> Option<WorkspaceId> {
    let id = window.window_handle().window_id();
    cx.try_global::<Windows>()?
        .entries
        .iter()
        .find(|e| e.window.window_id() == id)
        .map(|e| e.workspace)
}

/// The window showing workspace `id`.
pub fn for_workspace(id: WorkspaceId, cx: &App) -> Option<WindowEntry> {
    cx.try_global::<Windows>()?
        .entries
        .iter()
        .find(|e| e.workspace == id)
        .cloned()
}

/// The window a menu or key action is for: the active window when it is
/// one of ours, else the one of the focused workspace, else the main
/// window (the parity harness's window is never active).
pub fn focused(cx: &App) -> Option<WindowEntry> {
    let windows = cx.try_global::<Windows>()?;
    if let Some(active) = cx.active_window()
        && let Some(entry) = windows.entries.iter().find(|e| e.window == active)
    {
        return Some(entry.clone());
    }
    let focused = AppState::try_global(cx).map(|s| s.read(cx).focused);
    windows
        .entries
        .iter()
        .find(|e| Some(e.workspace) == focused)
        .or_else(|| windows.entries.first())
        .cloned()
}

/// The binary's window opener (see the module docs).
pub fn set_opener(opener: impl Fn(WorkspaceId, &mut App) + 'static, cx: &mut App) {
    registry(cx).opener = Some(Rc::new(opener));
}

/// Open a window for workspace `id` (made with
/// `Dispatcher::open_workspace`).
pub fn open_workspace_window(id: WorkspaceId, cx: &mut App) {
    let opener = cx.try_global::<Windows>().and_then(|w| w.opener.clone());
    if let Some(opener) = opener {
        // a menu action runs while its window is on the update stack
        // (`gpui-dispatch-traps`): open once GPUI has handed it back
        cx.defer(move |cx| opener(id, cx));
    }
}

/// Bring the window of workspace `id` forward (shown again when ⌘W hid it).
pub fn activate_workspace_window(id: WorkspaceId, cx: &mut App) {
    if let Some(entry) = for_workspace(id, cx) {
        entry
            .window
            .update(cx, |_, window, cx| {
                #[cfg(target_os = "macos")]
                crate::native_window::show_window(window, cx);
                window.activate_window();
                let _ = cx;
            })
            .ok();
    }
}

/// File › New Window (`429-multiple-windows`): an empty window whose
/// repository list opens so a repository can be picked.
pub fn new_window(cx: &mut App) {
    let id = Dispatcher::open_workspace(None, cx);
    open_workspace_window(id, cx);
    Dispatcher::toggle_foldout(corvene_core::Foldout::Repository, cx);
}

/// "Open in New Window" (`429-multiple-windows`): a window for `repo`, or
/// the one already showing it brought forward.
pub fn open_repository_in_new_window(repo: u64, cx: &mut App) {
    match Dispatcher::open_in_new_workspace(repo, cx) {
        Ok(id) => open_workspace_window(id, cx),
        Err(showing) => activate_workspace_window(showing, cx),
    }
}

/// A tab's "Open in New Window" (`430-repository-tabs`).
pub fn move_tab_to_new_window(repo: u64, cx: &mut App) {
    match Dispatcher::move_tab_to_new_workspace(repo, cx) {
        Ok(id) => open_workspace_window(id, cx),
        Err(showing) => activate_workspace_window(showing, cx),
    }
}

/// Run `f` with the state's current workspace set to the one `window`
/// shows (a view's observer, a control-socket command).
pub fn with_workspace_of<R>(window: &Window, cx: &mut App, f: impl FnOnce(&mut App) -> R) -> R {
    let workspace = workspace_of(window, cx);
    with_workspace(workspace, cx, f)
}

/// [`with_workspace_of`] by workspace id (`None`: the focused one).
pub fn with_workspace<R>(
    workspace: Option<WorkspaceId>,
    cx: &mut App,
    f: impl FnOnce(&mut App) -> R,
) -> R {
    if let Some(id) = workspace {
        Dispatcher::enter_workspace(id, cx);
    }
    let result = f(cx);
    Dispatcher::leave_workspace(cx);
    result
}

/// `cx.observe(&state, …)` for a view inside a window: the callback runs
/// with that window's workspace current, so `s.selected`, `s.popup()` and
/// the rest mean this window's (a second window's commit form would
/// otherwise follow the first window's repository).
pub fn observe_state<V: 'static>(
    state: &Entity<AppState>,
    cx: &mut Context<V>,
    mut f: impl FnMut(&mut V, Entity<AppState>, &mut Context<V>) + 'static,
) -> Subscription {
    cx.observe(state, move |this, state, cx| {
        let entity = cx.entity_id();
        let workspace = cx
            .with_window(entity, |window, cx| workspace_of(window, cx))
            .flatten();
        if let Some(id) = workspace {
            Dispatcher::enter_workspace(id, cx);
        }
        f(this, state, cx);
        Dispatcher::leave_workspace(cx);
    })
}

/// [`observe_state`] for `cx.observe_in(&state, window, …)`.
pub fn observe_state_in<V: 'static>(
    state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<V>,
    mut f: impl FnMut(&mut V, Entity<AppState>, &mut Window, &mut Context<V>) + 'static,
) -> Subscription {
    cx.observe_in(state, window, move |this, state, window, cx| {
        if let Some(id) = workspace_of(window, cx) {
            Dispatcher::enter_workspace(id, cx);
        }
        f(this, state, window, cx);
        Dispatcher::leave_workspace(cx);
    })
}

thread_local! {
    /// Per-window title cache for [`sync_window_title`]: set only on change
    /// (`WINDOW_TITLE` used to be one cell for the one window).
    static WINDOW_TITLES: RefCell<Vec<(WindowId, SharedString)>> = const { RefCell::new(Vec::new()) };
}

/// Set `window`'s title unless it already is `title` (macOS only).
pub fn sync_window_title(title: &SharedString, window: &mut Window) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let id = window.window_handle().window_id();
    let changed = WINDOW_TITLES.with(|titles| {
        let mut titles = titles.borrow_mut();
        match titles.iter_mut().find(|(w, _)| *w == id) {
            Some((_, current)) if *current == *title => false,
            Some((_, current)) => {
                *current = title.clone();
                true
            }
            None => {
                titles.push((id, title.clone()));
                true
            }
        }
    });
    if changed {
        window.set_window_title(title);
    }
}
