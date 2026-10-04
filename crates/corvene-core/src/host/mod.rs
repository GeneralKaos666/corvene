//! The runtime the dispatcher runs on: one [`AppState`], a main-thread
//! executor for the futures that apply results, a background executor for
//! git and disk work, and the few platform services GitHub Desktop reaches
//! through Electron (`dialog.showOpenDialog`, `shell.openExternal`,
//! `clipboard.writeText`, `shell.showItemInFolder`, `app.quit`).
//!
//! Every `Dispatcher` method takes `cx: &mut Ctx` where `Ctx = dyn Host`, so
//! GPUI's `App` and `Context<V>` (feature `gpui`, [`gpui`]) coerce at the
//! call site and the desktop UI passes its `cx` unchanged, while the Android
//! library drives the same dispatcher from a [`LocalHost`] run loop.

#[cfg(feature = "gpui")]
pub mod gpui;
#[cfg(any(target_os = "android", test, feature = "local-host"))]
pub mod local;
#[cfg(test)]
mod tests;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::state::AppState;

#[cfg(any(target_os = "android", test, feature = "local-host"))]
pub use local::{LocalHost, LoopHandle, spawn_loop};

/// What `Dispatcher` methods take, spelled `&mut dyn Host` in signatures so
/// the object lifetime follows the reference (GPUI's `Context<'a, V>` is not
/// `'static`).
pub type Ctx<'a> = dyn Host + 'a;

/// A future that runs on the host's main thread (it may touch `AppState`).
pub type LocalFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;
/// A future that runs on the host's background pool.
pub type SendFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// The runtime behind a `Ctx`. Object safe: the generic conveniences live
/// in `impl dyn Host` below.
pub trait Host {
    /// Stores the single `AppState`. Called once, by `Dispatcher::init`.
    fn install_state(&mut self, state: AppState);
    /// The state. Panics before `install_state`, as `App::global` does.
    fn state_ref(&self) -> &AppState;
    /// Mutates the state; a [`StateCx::notify`] inside `f` tells observers.
    fn update_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx));
    /// `true` once `install_state` ran.
    fn has_state(&self) -> bool;
    /// Runs `fut` on the main thread.
    fn spawn_local(&self, fut: LocalFuture);
    /// Runs `fut` on the background pool.
    fn spawn_background(&self, fut: SendFuture);
    /// Resolves after `after` (on the background executor).
    fn timer(&self, after: Duration) -> Task<()>;
    /// A `'static` handle back to this host for main-thread futures.
    fn async_ctx(&self) -> AsyncCtx;
    /// The platform services.
    fn services(&self) -> &dyn HostServices;
    /// The GPUI `App` behind this host, for UI callbacks that need their
    /// toolkit context back (`None` on hosts without GPUI).
    #[cfg(feature = "gpui")]
    fn gpui_app(&mut self) -> Option<&mut gpui_kit::App>;
}

/// The `cx` inside `state.update(cx, |s, cx| { …; cx.notify() })`.
#[derive(Default)]
pub struct StateCx {
    notified: bool,
}

impl StateCx {
    /// Observers (the desktop views, the Android `state_changed` event)
    /// learn that the state changed.
    pub fn notify(&mut self) {
        self.notified = true;
    }

    pub fn notified(&self) -> bool {
        self.notified
    }
}

/// Handle to the one `AppState`; `Copy` so the `let state = Self::state(cx)`
/// … `state.clone()` pattern in async closures keeps compiling.
#[derive(Clone, Copy, Default, Debug)]
pub struct StateHandle;

impl StateHandle {
    pub fn read<'a>(&self, cx: &'a dyn Host) -> &'a AppState {
        cx.state_ref()
    }

    /// `cx` is a `&mut Ctx`, or a `&mut AsyncCtx` inside a main-thread
    /// future (GPUI's `Entity::update` takes either too).
    pub fn update<C: StateAccess + ?Sized, R>(
        &self,
        cx: &mut C,
        f: impl FnOnce(&mut AppState, &mut StateCx) -> R,
    ) -> R {
        let mut f = Some(f);
        let mut out = None;
        cx.with_state(&mut |s, scx| {
            if let Some(f) = f.take() {
                out = Some(f(s, scx));
            }
        });
        take_result(out)
    }

    /// Reads the state from a main-thread future (GPUI `Entity::read_with`).
    pub fn read_with<R>(&self, cx: &AsyncCtx, f: impl FnOnce(&AppState, ()) -> R) -> R {
        cx.update(|cx| f(cx.state_ref(), ()))
    }
}

/// What [`StateHandle::update`] accepts.
pub trait StateAccess {
    fn with_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx));
}

impl StateAccess for dyn Host + '_ {
    fn with_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
        self.update_state(f);
    }
}

impl StateAccess for AsyncCtx {
    fn with_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
        self.0.with_host(&mut |cx| cx.update_state(f));
    }
}

/// `update_state` runs its closure exactly once; anything else is a host
/// bug, so this panics like `App::global` does before `init`.
#[track_caller]
fn take_result<R>(out: Option<R>) -> R {
    match out {
        Some(r) => r,
        None => panic!("the host did not run the state update"),
    }
}

/// A spawned background value (`cx.background_executor().spawn(..)`), or a
/// timer. Resolves on the main thread when awaited from a local future.
pub struct Task<T>(Pin<Box<dyn Future<Output = T> + Send + 'static>>);

impl<T> Task<T> {
    pub fn from_future(fut: impl Future<Output = T> + Send + 'static) -> Self {
        Task(Box::pin(fut))
    }

    pub fn ready(value: T) -> Self
    where
        T: Send + 'static,
    {
        Task(Box::pin(std::future::ready(value)))
    }

    /// The host already owns the work; dropping the handle changes nothing.
    pub fn detach(self) {}
}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.0.as_mut().poll(cx)
    }
}

/// `cx.background_executor()`: the two call shapes core uses.
pub struct BackgroundExecutor<'a>(&'a (dyn Host + 'a));

impl BackgroundExecutor<'_> {
    pub fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        spawn_background_task(self.0, fut)
    }

    pub fn timer(&self, after: Duration) -> Task<()> {
        self.0.timer(after)
    }
}

fn spawn_background_task<T: Send + 'static>(
    host: &(dyn Host + '_),
    fut: impl Future<Output = T> + Send + 'static,
) -> Task<T> {
    let (tx, rx) = async_channel::bounded::<T>(1);
    host.spawn_background(Box::pin(async move {
        let value = fut.await;
        let _ = tx.send(value).await;
    }));
    // A job that panicked never answers; the task then pends forever, which
    // is what GPUI's detached task does too.
    Task::from_future(async move {
        match rx.recv().await {
            Ok(value) => value,
            Err(_) => std::future::pending().await,
        }
    })
}

/// The result of `cx.spawn(..)`; `detach()` keeps the call sites as they
/// were under GPUI.
pub struct Spawned;

impl Spawned {
    pub fn detach(self) {}
}

impl dyn Host + '_ {
    pub fn background_executor(&self) -> BackgroundExecutor<'_> {
        BackgroundExecutor(self)
    }

    /// Runs `f` on the main thread; it gets an [`AsyncCtx`] to apply results.
    pub fn spawn<F>(&self, f: F) -> Spawned
    where
        F: AsyncFnOnce(&mut AsyncCtx) + 'static,
    {
        let mut acx = self.async_ctx();
        self.spawn_local(Box::pin(async move { f(&mut acx).await }));
        Spawned
    }

    pub fn prompt_for_paths(&self, options: PathPromptOptions) -> Task<Option<Vec<PathBuf>>> {
        self.services().prompt_for_paths(options)
    }

    pub fn write_to_clipboard(&self, text: String) {
        self.services().write_to_clipboard(text);
    }

    pub fn open_url(&self, url: &str) {
        self.services().open_url(url);
    }

    pub fn reveal_path(&self, path: &Path) {
        self.services().reveal_path(path);
    }

    pub fn open_with_system(&self, path: &Path) {
        self.services().open_with_system(path);
    }

    pub fn quit(&self) {
        self.services().quit();
    }
}

/// The `cx` inside `cx.spawn(async move |cx: &mut AsyncCtx| …)`: a
/// `'static` way back to the host from a main-thread future.
pub struct AsyncCtx(Box<dyn AsyncHost>);

impl Clone for AsyncCtx {
    fn clone(&self) -> Self {
        AsyncCtx(self.0.box_clone())
    }
}

/// What a host provides to make an [`AsyncCtx`].
pub trait AsyncHost: 'static {
    /// Runs `f` with the host, on the main thread.
    fn with_host(&self, f: &mut dyn FnMut(&mut dyn Host));
    fn box_clone(&self) -> Box<dyn AsyncHost>;
}

impl AsyncCtx {
    pub fn new(host: impl AsyncHost) -> Self {
        AsyncCtx(Box::new(host))
    }

    /// Applies `f` to the host (GPUI `AsyncApp::update`).
    pub fn update<R>(&self, f: impl FnOnce(&mut dyn Host) -> R) -> R {
        let mut f = Some(f);
        let mut out = None;
        self.0.with_host(&mut |cx| {
            if let Some(f) = f.take() {
                out = Some(f(cx));
            }
        });
        take_result(out)
    }

    pub fn background_executor(&self) -> OwnedBackgroundExecutor<'_> {
        OwnedBackgroundExecutor(self)
    }

    /// `cx.spawn(..)` from a callback that only holds an [`AsyncCtx`]
    /// (GPUI `AsyncApp::spawn`): `f` runs on the main thread.
    pub fn spawn<F>(&self, f: F) -> Spawned
    where
        F: AsyncFnOnce(&mut AsyncCtx) + 'static,
    {
        self.update(|cx| cx.spawn(f))
    }
}

/// `cx.background_executor()` on an [`AsyncCtx`].
pub struct OwnedBackgroundExecutor<'a>(&'a AsyncCtx);

impl OwnedBackgroundExecutor<'_> {
    pub fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        self.0.update(|cx| spawn_background_task(cx, fut))
    }

    pub fn timer(&self, after: Duration) -> Task<()> {
        self.0.update(|cx| cx.timer(after))
    }
}

/// `dialog.showOpenDialog` options.
#[derive(Clone, Debug, Default)]
pub struct PathPromptOptions {
    pub files: bool,
    pub directories: bool,
    pub multiple: bool,
    pub prompt: Option<String>,
}

/// Platform services that need the UI toolkit or the activity, so
/// `corvene_platform` cannot provide them.
pub trait HostServices {
    /// Resolves to the picked paths, or `None` when cancelled or failed.
    fn prompt_for_paths(&self, options: PathPromptOptions) -> Task<Option<Vec<PathBuf>>>;
    fn write_to_clipboard(&self, text: String);
    fn open_url(&self, url: &str);
    fn reveal_path(&self, path: &Path);
    fn open_with_system(&self, path: &Path);
    fn quit(&self);
}

/// Services that do nothing: tests, and hosts that have not wired theirs yet.
pub struct NoServices;

impl HostServices for NoServices {
    fn prompt_for_paths(&self, _options: PathPromptOptions) -> Task<Option<Vec<PathBuf>>> {
        Task::ready(None)
    }
    fn write_to_clipboard(&self, _text: String) {}
    fn open_url(&self, _url: &str) {}
    fn reveal_path(&self, _path: &Path) {}
    fn open_with_system(&self, _path: &Path) {}
    fn quit(&self) {}
}
