//! `AndroidPlatform`: the GPUI `Platform` for an Android activity.
//!
//! ```text
//! AndroidPlatform
//!   ├── AndroidDispatcher   ALooper foreground queue, worker pool, timers
//!   ├── CosmicTextSystem    cosmic-text + swash over /system/fonts
//!   ├── GpuContext          shared wgpu device, created with the first surface
//!   └── AndroidWindow       the activity's window, across surface re-creations
//! ```
//!
//! `run` blocks in the event loop (`event_loop.rs`) for the life of the
//! activity. The system creates the native window, so the finish-launching
//! callback runs once the first surface exists and `open_window` adopts it.
//!
//! Ported from gpui-mobile's `src/android/platform.rs`, updated from Zed's
//! GPUI of April 2026 to the `Platform` trait of gpui-pre 0.3.7 (restart
//! arguments, `on_quit` vetoes, sleep / wake, application lifecycle, memory
//! warnings, gestures, cursor visibility, idle sleep, …). The platform is a
//! single-threaded `Rc` value, where gpui-mobile kept an `Arc<Mutex<…>>`
//! global behind a `SharedPlatform` wrapper; its stubbed clipboard and URL
//! opening are real JNI calls.

use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use android_activity::{AndroidApp, AndroidAppWaker};
use anyhow::{anyhow, Result};
use futures::channel::oneshot;
use gpui::{
    Action, ActivityGuard, AnyWindowHandle, AppLifecyclePhase, BackgroundExecutor, ClipboardItem,
    CursorStyle, DummyKeyboardMapper, ForegroundExecutor, GestureTuning, Keymap, Menu, MenuItem,
    OwnedMenu, PathPromptOptions, Platform, PlatformDisplay, PlatformGestures,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow,
    PriorityQueueReceiver, RunnableVariant, ScrollPhysics, Task, ThermalState, WindowAppearance,
    WindowParams,
};
use gpui_wgpu::{CosmicTextSystem, GpuContext};

use super::{
    dispatcher::AndroidDispatcher,
    display::scale_factor_for_density,
    jni,
    keyboard::AndroidKeyboardLayout,
    window::{AndroidPlatformWindow, AndroidWindow},
};

#[derive(Default)]
pub(crate) struct PlatformCallbacks {
    pub(crate) open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    pub(crate) quit: Option<Box<dyn FnMut() -> bool>>,
    pub(crate) reopen: Option<Box<dyn FnMut()>>,
    pub(crate) app_lifecycle: Option<Box<dyn FnMut(AppLifecyclePhase)>>,
    pub(crate) memory_warning: Option<Box<dyn FnMut()>>,
    pub(crate) keyboard_layout_change: Option<Box<dyn FnMut()>>,
    pub(crate) app_menu_action: Option<Box<dyn FnMut(&dyn Action)>>,
    pub(crate) will_open_app_menu: Option<Box<dyn FnMut()>>,
    pub(crate) validate_app_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
}

pub struct AndroidPlatform {
    pub(crate) app: AndroidApp,
    /// Foreground tasks, run by the event loop.
    pub(crate) main_receiver: PriorityQueueReceiver<RunnableVariant>,
    /// Makes `poll_events` return: a frame or a quit was asked for while the
    /// loop was about to block.
    pub(crate) waker: AndroidAppWaker,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<CosmicTextSystem>,
    gpu_context: GpuContext,
    /// The activity's window, once the first surface exists.
    pub(crate) window: RefCell<Option<Rc<AndroidWindow>>>,
    /// The GPUI handle that adopted the window.
    window_handle: Cell<Option<AnyWindowHandle>>,
    pub(crate) callbacks: RefCell<PlatformCallbacks>,
    pub(crate) finish_launching: RefCell<Option<Box<dyn FnOnce()>>>,
    pub(crate) should_quit: Cell<bool>,
    /// Whether the activity's window has input focus; it may gain it before
    /// its first surface exists.
    pub(crate) focused: Cell<bool>,
    menus: RefCell<Vec<OwnedMenu>>,
    /// URLs that arrived before `on_open_urls` was registered.
    pending_urls: RefCell<Vec<String>>,
}

impl AndroidPlatform {
    /// Must be called on the `android_main` thread.
    pub fn new(app: AndroidApp) -> Self {
        super::init_logger();
        jni::init(&app);

        let (dispatcher, main_receiver) = AndroidDispatcher::new(&app);
        let text_system = Arc::new(CosmicTextSystem::new("Roboto"));
        text_system.load_fonts_dir(Path::new("/system/fonts"));
        // fonts an OEM or a module adds
        text_system.load_fonts_dir(Path::new("/product/fonts"));

        let pending_urls = jni::launch_url().into_iter().collect();
        Self {
            waker: app.create_waker(),
            app,
            main_receiver,
            background_executor: BackgroundExecutor::new(dispatcher.clone()),
            foreground_executor: ForegroundExecutor::new(dispatcher),
            text_system,
            gpu_context: GpuContext::default(),
            window: RefCell::new(None),
            window_handle: Cell::new(None),
            callbacks: RefCell::new(PlatformCallbacks::default()),
            finish_launching: RefCell::new(None),
            should_quit: Cell::new(false),
            focused: Cell::new(false),
            menus: RefCell::new(Vec::new()),
            pending_urls: RefCell::new(pending_urls),
        }
    }

    pub(crate) fn window(&self) -> Option<Rc<AndroidWindow>> {
        self.window.borrow().clone()
    }

    pub(crate) fn gpu_context(&self) -> GpuContext {
        self.gpu_context.clone()
    }

    /// The scale factor the configuration's density asks for.
    pub(crate) fn scale_factor(&self) -> f32 {
        scale_factor_for_density(self.app.config().density())
    }

    pub(crate) fn appearance(&self) -> WindowAppearance {
        match self.app.config().ui_mode_night() {
            ndk::configuration::UiModeNight::Yes => WindowAppearance::Dark,
            _ => WindowAppearance::Light,
        }
    }

    /// Hands URLs (a deep link, `onNewIntent`) to the application.
    pub fn open_urls(&self, urls: Vec<String>) {
        let callback = self.callbacks.borrow_mut().open_urls.take();
        match callback {
            Some(mut callback) => {
                callback(urls);
                self.callbacks.borrow_mut().open_urls = Some(callback);
            }
            None => self.pending_urls.borrow_mut().extend(urls),
        }
    }

    pub(crate) fn fire_lifecycle(&self, phase: AppLifecyclePhase) {
        let callback = self.callbacks.borrow_mut().app_lifecycle.take();
        if let Some(mut callback) = callback {
            callback(phase);
            self.callbacks.borrow_mut().app_lifecycle = Some(callback);
        }
    }

    pub(crate) fn fire_memory_warning(&self) {
        let callback = self.callbacks.borrow_mut().memory_warning.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks.borrow_mut().memory_warning = Some(callback);
        }
    }

    pub(crate) fn fire_keyboard_layout_change(&self) {
        let callback = self.callbacks.borrow_mut().keyboard_layout_change.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks.borrow_mut().keyboard_layout_change = Some(callback);
        }
    }
}

struct AndroidGestures;

impl PlatformGestures for AndroidGestures {
    fn tuning(&self) -> GestureTuning {
        GestureTuning {
            scroll_physics: ScrollPhysics::android(),
            ..GestureTuning::default()
        }
    }
}

impl Platform for AndroidPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        *self.finish_launching.borrow_mut() = Some(on_finish_launching);
        self.run_event_loop();
    }

    fn quit(&self) {
        self.should_quit.set(true);
        self.waker.wake();
    }

    fn restart(&self, _binary_path: Option<PathBuf>, _arguments: Vec<OsString>) {
        log::warn!("restart is not supported on Android");
    }

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.primary_display().into_iter().collect()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.window()
            .map(|window| Rc::new(window.display()) as Rc<dyn PlatformDisplay>)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.window_handle.get()
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        _options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        // The system creates the activity's one window; GPUI adopts it.
        let window = self
            .window()
            .ok_or_else(|| anyhow!("the activity has no window yet"))?;
        if self.window_handle.get().is_some() {
            return Err(anyhow!("an Android activity has a single window"));
        }
        self.window_handle.set(Some(handle));
        Ok(Box::new(AndroidPlatformWindow(window)))
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.appearance()
    }

    fn open_url(&self, url: &str) {
        jni::open_url(url);
    }

    fn on_open_urls(&self, mut callback: Box<dyn FnMut(Vec<String>)>) {
        let pending = std::mem::take(&mut *self.pending_urls.borrow_mut());
        if !pending.is_empty() {
            callback(pending);
        }
        self.callbacks.borrow_mut().open_urls = Some(callback);
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        // schemes are declared in the manifest's intent filters
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        super::activity_events::prompt_for_paths(options)
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {}

    fn open_with_system(&self, _path: &Path) {}

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.callbacks.borrow_mut().quit = Some(callback);
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().reopen = Some(callback);
    }

    fn on_system_sleep(&self, _callback: Box<dyn FnMut()>) {}

    fn on_system_wake(&self, _callback: Box<dyn FnMut()>) {}

    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.callbacks.borrow_mut().app_lifecycle = Some(callback);
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().memory_warning = Some(callback);
    }

    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        Some(Rc::new(AndroidGestures))
    }

    fn set_menus(&self, menus: Vec<Menu>, _keymap: &Keymap) {
        *self.menus.borrow_mut() = menus.into_iter().map(|menu| menu.owned()).collect();
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        Some(self.menus.borrow().clone())
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.callbacks.borrow_mut().app_menu_action = Some(callback);
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().will_open_app_menu = Some(callback);
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.callbacks.borrow_mut().validate_app_menu_command = Some(callback);
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}

    fn prevent_idle_sleep(&self, _reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Ok(ActivityGuard::noop()))
    }

    fn compositor_name(&self) -> &'static str {
        "Android"
    }

    fn app_path(&self) -> Result<PathBuf> {
        Err(anyhow!("an Android application has no executable path"))
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        Err(anyhow!(
            "auxiliary executables are not supported on Android"
        ))
    }

    fn set_cursor_style(&self, _style: CursorStyle) {}

    fn hide_cursor_until_mouse_moves(&self) {}

    fn is_cursor_visible(&self) -> bool {
        true
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        jni::clipboard_text().map(ClipboardItem::new_string)
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        jni::set_clipboard_text(&item.text().unwrap_or_default());
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow!(
            "GPUI credentials are not implemented on Android"
        )))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(AndroidKeyboardLayout::new("android"))
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().keyboard_layout_change = Some(callback);
    }
}
