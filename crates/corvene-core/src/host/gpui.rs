//! GPUI as a [`Host`]: the state lives in an `Entity<AppState>` installed
//! as a global so the desktop views keep `AppState::global(cx)`,
//! `state.read(cx)` and `cx.observe(&state, …)`; a `StateCx::notify` inside
//! an update becomes the entity's `cx.notify()`.

use std::borrow::BorrowMut;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::{App, AppContext, AsyncApp, Context, Entity, Global};

use super::{
    AsyncCtx, AsyncHost, Host, HostServices, LocalFuture, PathPromptOptions, SendFuture, StateCx,
    Task,
};
use crate::state::AppState;

struct AppStateHandle(Entity<AppState>);
impl Global for AppStateHandle {}

impl AppState {
    /// The single app-state entity. Panics if `Dispatcher::init` has not run.
    pub fn global(cx: &App) -> Entity<AppState> {
        cx.global::<AppStateHandle>().0.clone()
    }

    /// `None` before `Dispatcher::init` (widgets rendered in isolation).
    pub fn try_global(cx: &App) -> Option<Entity<AppState>> {
        cx.try_global::<AppStateHandle>().map(|h| h.0.clone())
    }
}

fn install_state(app: &mut App, state: AppState) {
    let entity = app.new(|_| state);
    app.set_global(AppStateHandle(entity));
}

fn state_ref(app: &App) -> &AppState {
    app.global::<AppStateHandle>().0.read(app)
}

fn update_state(app: &mut App, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
    let entity = AppState::global(app);
    entity.update(app, |s, gcx| {
        let mut scx = StateCx::default();
        f(s, &mut scx);
        if scx.notified() {
            gcx.notify();
        }
    });
}

fn spawn_local(app: &App, fut: LocalFuture) {
    app.spawn(async move |_| fut.await).detach();
}

fn spawn_background(app: &App, fut: SendFuture) {
    app.background_executor().spawn(fut).detach();
}

fn timer(app: &App, after: Duration) -> Task<()> {
    Task::from_future(app.background_executor().timer(after))
}

impl Host for App {
    fn install_state(&mut self, state: AppState) {
        install_state(self, state);
    }
    fn state_ref(&self) -> &AppState {
        state_ref(self)
    }
    fn update_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
        update_state(self, f);
    }
    fn has_state(&self) -> bool {
        self.try_global::<AppStateHandle>().is_some()
    }
    fn spawn_local(&self, fut: LocalFuture) {
        spawn_local(self, fut);
    }
    fn spawn_background(&self, fut: SendFuture) {
        spawn_background(self, fut);
    }
    fn timer(&self, after: Duration) -> Task<()> {
        timer(self, after)
    }
    fn async_ctx(&self) -> AsyncCtx {
        AsyncCtx::new(self.to_async())
    }
    fn services(&self) -> &dyn HostServices {
        self
    }
    fn gpui_app(&mut self) -> Option<&mut App> {
        Some(self)
    }
}

impl<V: 'static> Host for Context<'_, V> {
    fn install_state(&mut self, state: AppState) {
        install_state(self.borrow_mut(), state);
    }
    fn state_ref(&self) -> &AppState {
        state_ref(self)
    }
    fn update_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
        update_state(self.borrow_mut(), f);
    }
    fn has_state(&self) -> bool {
        self.try_global::<AppStateHandle>().is_some()
    }
    fn spawn_local(&self, fut: LocalFuture) {
        spawn_local(self, fut);
    }
    fn spawn_background(&self, fut: SendFuture) {
        spawn_background(self, fut);
    }
    fn timer(&self, after: Duration) -> Task<()> {
        timer(self, after)
    }
    fn async_ctx(&self) -> AsyncCtx {
        AsyncCtx::new(self.to_async())
    }
    fn services(&self) -> &dyn HostServices {
        let app: &App = self;
        app
    }
    fn gpui_app(&mut self) -> Option<&mut App> {
        Some(self.borrow_mut())
    }
}

impl AsyncHost for AsyncApp {
    fn with_host(&self, f: &mut dyn FnMut(&mut dyn Host)) {
        self.update(|app| f(app));
    }
    fn box_clone(&self) -> Box<dyn AsyncHost> {
        Box::new(self.clone())
    }
}

impl HostServices for App {
    fn prompt_for_paths(&self, options: PathPromptOptions) -> Task<Option<Vec<PathBuf>>> {
        let receiver = App::prompt_for_paths(
            self,
            gpui_kit::PathPromptOptions {
                files: options.files,
                directories: options.directories,
                multiple: options.multiple,
                prompt: options.prompt.map(Into::into),
            },
        );
        Task::from_future(async move {
            match receiver.await {
                Ok(Ok(paths)) => paths,
                _ => None,
            }
        })
    }

    fn write_to_clipboard(&self, text: String) {
        App::write_to_clipboard(self, gpui_kit::ClipboardItem::new_string(text));
    }

    fn open_url(&self, url: &str) {
        App::open_url(self, url);
    }

    fn reveal_path(&self, path: &Path) {
        App::reveal_path(self, path);
    }

    fn open_with_system(&self, path: &Path) {
        App::open_with_system(self, path);
    }

    fn quit(&self) {
        App::quit(self);
    }
}
