//! Android window implementation.
//!
//! An activity has one `ANativeWindow`, handed over (and taken away again) by
//! the system: `AndroidWindow` is the logical GPUI window that outlives those
//! surfaces. It owns the `WgpuRenderer`, whose surface is replaced when the
//! activity comes back, so the atlas and GPUI's scene cache stay valid.
//!
//! Ported from gpui-mobile's `src/android/window.rs`. gpui-pre 0.3.7
//! differences:
//!
//! * Touches are forwarded as `PlatformInput::Touch`; GPUI's gesture arena
//!   turns them into taps, long presses, pans and momentum (gpui-mobile
//!   emulated a mouse and ran its own fling physics).
//! * Mice, touchpads and styluses with buttons (Chromebooks, DeX) produce
//!   real mouse events with hover, right click and the wheel.
//! * Frames are drawn on demand (`frame_waker` / `schedule_frame`), so an
//!   idle window costs nothing.
//! * Everything lives on the `android_main` thread behind `Rc` / `RefCell`;
//!   gpui-mobile shared `Arc<Mutex<…>>` globals and transmuted callbacks to
//!   `Send`.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ptr::NonNull,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use android_activity::{AndroidApp, AndroidAppWaker};
use anyhow::{Context as _, Result};
use futures::channel::oneshot;
use gpui::{
    point, px, size, Bounds, Capslock, DevicePixels, DispatchEventResult, Edges, GpuSpecs,
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PlatformAtlas, PlatformDisplay,
    PlatformInput, PlatformInputHandler, PlatformWindow, Point, PromptButton, PromptLevel,
    RequestFrameOptions, Scene, Size, TouchEvent, TouchId, TouchPhase, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowControls, WindowInsets,
    WindowVisibility,
};
use gpui_wgpu::{wgpu, GpuContext, WgpuRenderer, WgpuSurfaceConfig};
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use super::display::AndroidDisplay;

/// Two presses of a mouse button count as a double click within this time
/// and distance (`ViewConfiguration.getDoubleTapTimeout()` is 300 ms; mice
/// get the desktop's more forgiving 500 ms).
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const MULTI_CLICK_SLOP: f32 = 4.0;

/// `ViewConfiguration.getLongPressTimeout()` and `getScaledTouchSlop()`'s
/// defaults (500 ms, 8 dp).
const LONG_PRESS: Duration = Duration::from_millis(500);
const TOUCH_SLOP: f32 = 8.0;

/// How far above or below the caret's line a tap still counts as a tap on
/// the text input, in logical pixels.
const KEYBOARD_TAP_MARGIN: f32 = 24.0;

/// An owned window handle for wgpu, which needs `Clone + Send + Sync`.
#[derive(Debug, Clone, Copy)]
struct RawAndroidWindow {
    native_window: NonNull<std::ffi::c_void>,
}

// SAFETY: the pointer is only dereferenced by the Vulkan / EGL surface
// creation on the main thread, while `WindowState::native_window` keeps the
// `ANativeWindow` alive.
unsafe impl Send for RawAndroidWindow {}
unsafe impl Sync for RawAndroidWindow {}

impl RawAndroidWindow {
    fn new(native_window: &NativeWindow) -> Self {
        Self {
            native_window: native_window.ptr().cast(),
        }
    }
}

impl HasWindowHandle for RawAndroidWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        let handle = AndroidNdkWindowHandle::new(self.native_window);
        // SAFETY: see the `Send` note above
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl HasDisplayHandle for RawAndroidWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        // SAFETY: the Android display handle carries no pointer
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Android(
                AndroidDisplayHandle::new(),
            ))
        })
    }
}

#[derive(Default)]
struct Callbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    visibility_change: Option<Box<dyn FnMut(WindowVisibility)>>,
    hover_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    insets_changed: Option<Box<dyn FnMut(WindowInsets)>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    back: Option<Box<dyn FnMut()>>,
}

struct LastClick {
    button: MouseButton,
    at: Instant,
    position: Point<Pixels>,
    count: usize,
}

struct WindowState {
    /// `None` between `TerminateWindow` and the next `InitWindow`.
    native_window: Option<NativeWindow>,
    gpu_context: GpuContext,
    renderer: WgpuRenderer,
    size: Size<DevicePixels>,
    scale_factor: f32,
    insets: WindowInsets,
    appearance: WindowAppearance,
    active: bool,
    hovered: bool,
    input_handler: Option<PlatformInputHandler>,
    modifiers: Modifiers,
    capslock: Capslock,
    mouse_position: Point<Pixels>,
    pressed_button: Option<MouseButton>,
    last_click: Option<LastClick>,
    /// Android reuses pointer ids; GPUI wants an id per touch.
    touches: HashMap<i32, TouchId>,
    next_touch_id: u64,
    title: String,
    back_enabled: bool,
    /// A finger resting where it went down: a long press in the making.
    long_press: Option<LongPress>,
    /// A tap that ended; after the next frame the keyboard opens if the tap
    /// was on the focused text input.
    tapped: Option<Point<Pixels>>,
}

struct LongPress {
    pointer_id: i32,
    position: Point<Pixels>,
    deadline: Instant,
}

pub(crate) struct AndroidWindow {
    app: AndroidApp,
    state: RefCell<WindowState>,
    callbacks: RefCell<Callbacks>,
    /// GPUI asked for a frame (`frame_waker`, `schedule_frame`) or the
    /// surface changed.
    frame_requested: Cell<bool>,
    /// The next frame must be rendered and presented whatever GPUI thinks is
    /// dirty: the surface is new or was resized.
    force_render: Cell<bool>,
    waker: AndroidAppWaker,
}

fn surface_config(size: Size<DevicePixels>) -> WgpuSurfaceConfig {
    WgpuSurfaceConfig {
        size,
        transparent: false,
        // vsync: a frame is only drawn on demand, so there is no reason to
        // render faster than the display shows
        preferred_present_mode: Some(wgpu::PresentMode::Fifo),
    }
}

fn window_size(native_window: &NativeWindow) -> Size<DevicePixels> {
    size(
        DevicePixels(native_window.width().max(1)),
        DevicePixels(native_window.height().max(1)),
    )
}

impl AndroidWindow {
    pub(crate) fn new(
        app: AndroidApp,
        native_window: NativeWindow,
        gpu_context: GpuContext,
        scale_factor: f32,
        appearance: WindowAppearance,
    ) -> Result<Rc<Self>> {
        let size = window_size(&native_window);
        let create = || {
            WgpuRenderer::new(
                gpu_context.clone(),
                &RawAndroidWindow::new(&native_window),
                surface_config(size),
                None,
            )
        };
        let mut renderer = create();
        // Some Vulkan drivers (the emulator's) accept the surface and then
        // lose the device while the pipelines are built: start over on
        // OpenGL ES.
        let vulkan_failed = match &renderer {
            Ok(renderer) => renderer.device_lost(),
            Err(_) => true,
        };
        if vulkan_failed && !gpui_wgpu::ANDROID_GL_ONLY.swap(true, Ordering::Relaxed) {
            log::warn!("Vulkan is not usable on this device; using OpenGL ES");
            if let Ok(mut renderer) = renderer {
                renderer.destroy();
            }
            *gpu_context.borrow_mut() = None;
            renderer = create();
        }
        let renderer = renderer.context("failed to create the wgpu renderer")?;
        log::info!(
            "window {}×{} at {scale_factor}×",
            size.width.0,
            size.height.0
        );

        let window = Rc::new(Self {
            waker: app.create_waker(),
            app,
            state: RefCell::new(WindowState {
                native_window: Some(native_window),
                gpu_context,
                renderer,
                size,
                scale_factor,
                insets: WindowInsets::default(),
                appearance,
                active: false,
                hovered: false,
                input_handler: None,
                modifiers: Modifiers::default(),
                capslock: Capslock::default(),
                mouse_position: Point::default(),
                pressed_button: None,
                last_click: None,
                touches: HashMap::new(),
                next_touch_id: 1,
                title: String::new(),
                back_enabled: false,
                long_press: None,
                tapped: None,
            }),
            callbacks: RefCell::new(Callbacks::default()),
            frame_requested: Cell::new(true),
            force_render: Cell::new(true),
        });
        window.update_insets();
        Ok(window)
    }

    // ── surface lifecycle ────────────────────────────────────────────────────

    /// `InitWindow` for a window that already exists: the activity came back.
    pub(crate) fn surface_created(&self, native_window: NativeWindow, scale_factor: f32) {
        let size = window_size(&native_window);
        let changed = {
            let mut state = self.state.borrow_mut();
            let instance = state
                .gpu_context
                .borrow()
                .as_ref()
                .map(|context| context.instance.clone());
            let replaced = match instance {
                Some(instance) => state.renderer.replace_surface(
                    &RawAndroidWindow::new(&native_window),
                    surface_config(size),
                    &instance,
                ),
                None => Err(anyhow::anyhow!("no GPU context")),
            };
            if let Err(err) = replaced {
                log::error!("failed to replace the surface: {err:#}");
            }
            state.native_window = Some(native_window);
            let changed = state.size != size || state.scale_factor != scale_factor;
            state.size = size;
            state.scale_factor = scale_factor;
            changed
        };
        self.update_insets();
        if changed {
            self.fire_resize();
        }
        self.fire_visibility(WindowVisibility::Visible);
        self.request_forced_frame();
    }

    /// `TerminateWindow`: the surface is going away; keep the renderer.
    pub(crate) fn surface_destroyed(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.renderer.unconfigure_surface();
            state.native_window = None;
            state.touches.clear();
        }
        self.fire_visibility(WindowVisibility::Hidden);
    }

    /// `WindowResized`, `ContentRectChanged` or a density change.
    pub(crate) fn surface_changed(&self, scale_factor: f32) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let Some(native_window) = state.native_window.as_ref() else {
                return;
            };
            let size = window_size(native_window);
            let changed = state.size != size || state.scale_factor != scale_factor;
            if state.size != size {
                state.renderer.update_drawable_size(size);
            }
            state.size = size;
            state.scale_factor = scale_factor;
            changed
        };
        let insets_changed = self.update_insets();
        if changed {
            self.fire_resize();
        }
        if changed || insets_changed {
            self.request_forced_frame();
        }
    }

    pub(crate) fn has_surface(&self) -> bool {
        self.state.borrow().native_window.is_some()
    }

    /// Reads the content rectangle (the part of the window the system bars
    /// leave free) into the safe area; true when it changed.
    fn update_insets(&self) -> bool {
        let rect = self.app.content_rect();
        let insets = {
            let state = self.state.borrow();
            let scale = state.scale_factor;
            let (width, height) = (state.size.width.0, state.size.height.0);
            // an empty rectangle means the system has not reported one yet
            if rect.right <= rect.left || rect.bottom <= rect.top {
                WindowInsets::default()
            } else {
                WindowInsets {
                    safe_area: Edges {
                        top: px(rect.top.max(0) as f32 / scale),
                        left: px(rect.left.max(0) as f32 / scale),
                        right: px((width - rect.right).max(0) as f32 / scale),
                        bottom: px((height - rect.bottom).max(0) as f32 / scale),
                    },
                    ime: state.insets.ime,
                }
            }
        };
        if self.state.borrow().insets == insets {
            return false;
        }
        self.state.borrow_mut().insets = insets.clone();
        let callback = self.callbacks.borrow_mut().insets_changed.take();
        if let Some(mut callback) = callback {
            callback(insets);
            self.callbacks.borrow_mut().insets_changed = Some(callback);
        }
        true
    }

    fn fire_resize(&self) {
        let (size, scale) = {
            let state = self.state.borrow();
            (logical_size(&state), state.scale_factor)
        };
        let callback = self.callbacks.borrow_mut().resize.take();
        if let Some(mut callback) = callback {
            callback(size, scale);
            self.callbacks.borrow_mut().resize = Some(callback);
        }
    }

    fn fire_visibility(&self, visibility: WindowVisibility) {
        let callback = self.callbacks.borrow_mut().visibility_change.take();
        if let Some(mut callback) = callback {
            callback(visibility);
            self.callbacks.borrow_mut().visibility_change = Some(callback);
        }
    }

    // ── activity state ───────────────────────────────────────────────────────

    pub(crate) fn set_active(&self, active: bool) {
        if std::mem::replace(&mut self.state.borrow_mut().active, active) == active {
            return;
        }
        if !active {
            let mut state = self.state.borrow_mut();
            state.modifiers = Modifiers::default();
            state.pressed_button = None;
        }
        let callback = self.callbacks.borrow_mut().active_status_change.take();
        if let Some(mut callback) = callback {
            callback(active);
            self.callbacks.borrow_mut().active_status_change = Some(callback);
        }
    }

    pub(crate) fn set_appearance(&self, appearance: WindowAppearance) {
        if std::mem::replace(&mut self.state.borrow_mut().appearance, appearance) == appearance {
            return;
        }
        let callback = self.callbacks.borrow_mut().appearance_changed.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks.borrow_mut().appearance_changed = Some(callback);
        }
    }

    pub(crate) fn display(&self) -> AndroidDisplay {
        let state = self.state.borrow();
        AndroidDisplay::new(state.size, state.scale_factor)
    }

    /// The activity is being destroyed.
    pub(crate) fn close(&self) {
        let callback = self.callbacks.borrow_mut().close.take();
        if let Some(callback) = callback {
            callback();
        }
        self.state.borrow_mut().renderer.destroy();
    }

    // ── frames ───────────────────────────────────────────────────────────────

    fn request_forced_frame(&self) {
        self.force_render.set(true);
        self.request_frame();
    }

    /// Asks the event loop for a frame; it may be about to block.
    fn request_frame(&self) {
        if !self.frame_requested.replace(true) {
            self.waker.wake();
        }
    }

    /// The system asked for the window to be redrawn (`RedrawNeeded`).
    pub(crate) fn redraw(&self) {
        self.request_forced_frame();
    }

    /// Whether the event loop should wake for the next vsync.
    pub(crate) fn wants_frame(&self) -> bool {
        self.frame_requested.get() && self.has_surface()
    }

    /// One frame: GPUI draws when its window is dirty and asks again (through
    /// the waker or `schedule_frame`) while it has more to show.
    pub(crate) fn frame(&self) {
        self.frame_requested.set(false);
        let force = self.force_render.take();
        let callback = self.callbacks.borrow_mut().request_frame.take();
        if let Some(mut callback) = callback {
            callback(RequestFrameOptions {
                require_presentation: force,
                force_render: force,
            });
            self.callbacks.borrow_mut().request_frame = Some(callback);
        }
        self.show_keyboard_for_tap();
    }

    // ── touch conventions ────────────────────────────────────────────────────

    /// A finger went down at `position`: a long press starts counting.
    pub(crate) fn long_press_started(&self, pointer_id: i32, position: Point<Pixels>) {
        self.state.borrow_mut().long_press = Some(LongPress {
            pointer_id,
            position,
            deadline: Instant::now() + LONG_PRESS,
        });
    }

    /// The finger moved: past the slop it is a pan, not a press.
    pub(crate) fn long_press_moved(&self, pointer_id: i32, position: Point<Pixels>) {
        let mut state = self.state.borrow_mut();
        let moved_away = state.long_press.as_ref().is_some_and(|press| {
            press.pointer_id == pointer_id
                && (f32::from((press.position.x - position.x).abs()) > TOUCH_SLOP
                    || f32::from((press.position.y - position.y).abs()) > TOUCH_SLOP)
        });
        if moved_away {
            state.long_press = None;
        }
    }

    /// The finger lifted, another came down or the gesture was cancelled.
    /// Returns where the finger was when it lifted in place before the long
    /// press was due: a tap.
    pub(crate) fn long_press_ended(&self, tap: bool) -> Option<Point<Pixels>> {
        let press = self.state.borrow_mut().long_press.take()?;
        tap.then_some(press.position)
    }

    /// When the event loop must wake for a long press.
    pub(crate) fn long_press_deadline(&self) -> Option<Instant> {
        self.state
            .borrow()
            .long_press
            .as_ref()
            .map(|press| press.deadline)
    }

    /// Android's convention: a long press is the secondary click. The touch
    /// is cancelled (lifting the finger must not tap) and a right-button
    /// press and release go where the finger rests, which opens the context
    /// menu of whatever is there.
    pub(crate) fn fire_long_press(&self) {
        let due = self
            .state
            .borrow()
            .long_press
            .as_ref()
            .is_some_and(|press| press.deadline <= Instant::now());
        if !due {
            return;
        }
        let Some(press) = self.state.borrow_mut().long_press.take() else {
            return;
        };
        if let Some(id) = self.touch_ended(press.pointer_id) {
            self.handle_input(PlatformInput::Touch(TouchEvent {
                id,
                phase: TouchPhase::Cancelled,
                position: press.position,
                predicted_position: None,
                force: None,
            }));
        }
        self.set_mouse_position(press.position);
        self.handle_input(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Right,
            position: press.position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.handle_input(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Right,
            position: press.position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    /// A tap ended at `position`; see [`Self::show_keyboard_for_tap`].
    pub(crate) fn tapped(&self, position: Point<Pixels>) {
        self.state.borrow_mut().tapped = Some(position);
        self.request_frame();
    }

    /// The on-screen keyboard opens when a text input is tapped, not
    /// whenever one takes focus: lists and dialogs focus their filter box on
    /// their own, and a keyboard over half the screen each time would be in
    /// the way. Checked after the frame that follows a tap, when the tapped
    /// input has focus and its caret is where the tap was.
    fn show_keyboard_for_tap(&self) {
        let Some(tap) = self.state.borrow_mut().tapped.take() else {
            return;
        };
        let Some(mut handler) = self.state.borrow_mut().input_handler.take() else {
            return;
        };
        let caret = handler
            .selected_text_range(true)
            .and_then(|selection| handler.bounds_for_range(selection.range));
        self.state.borrow_mut().input_handler = Some(handler);
        let on_input = caret.is_some_and(|caret| {
            f32::from(tap.y) >= f32::from(caret.top()) - KEYBOARD_TAP_MARGIN
                && f32::from(tap.y) <= f32::from(caret.bottom()) + KEYBOARD_TAP_MARGIN
        });
        if on_input {
            super::activity_events::show_soft_keyboard(true);
        }
    }

    // ── input ────────────────────────────────────────────────────────────────

    /// Hands `input` to GPUI; true when a handler took it.
    pub(crate) fn handle_input(&self, input: PlatformInput) -> bool {
        let callback = self.callbacks.borrow_mut().input.take();
        let mut handled = false;
        if let Some(mut callback) = callback {
            let result = callback(input.clone());
            self.callbacks.borrow_mut().input = Some(callback);
            handled = !result.propagate || result.default_prevented;
            if !result.propagate {
                return true;
            }
        }
        // a key no binding took types its character, as on X11
        if let PlatformInput::KeyDown(event) = input {
            if event.keystroke.modifiers.is_subset_of(&Modifiers::shift()) {
                if let Some(key_char) = &event.keystroke.key_char {
                    if self.insert_text(key_char) {
                        return true;
                    }
                }
            }
        }
        handled
    }

    /// Types `text` into the focused text input; false without one.
    pub(crate) fn insert_text(&self, text: &str) -> bool {
        let handler = self.state.borrow_mut().input_handler.take();
        match handler {
            Some(mut handler) => {
                handler.replace_text_in_range(None, text);
                self.state.borrow_mut().input_handler = Some(handler);
                true
            }
            None => false,
        }
    }

    /// Shows `text` as the composition in progress (marked text).
    pub(crate) fn set_composing_text(&self, text: &str) {
        let handler = self.state.borrow_mut().input_handler.take();
        if let Some(mut handler) = handler {
            handler.replace_and_mark_text_in_range(None, text, None);
            self.state.borrow_mut().input_handler = Some(handler);
        }
    }

    pub(crate) fn finish_composing(&self) {
        let handler = self.state.borrow_mut().input_handler.take();
        if let Some(mut handler) = handler {
            handler.unmark_text();
            self.state.borrow_mut().input_handler = Some(handler);
        }
    }

    pub(crate) fn logical_point(&self, x: f32, y: f32) -> Point<Pixels> {
        let scale = self.state.borrow().scale_factor;
        point(px(x / scale), px(y / scale))
    }

    /// The GPUI id of an Android pointer, assigned when the touch starts.
    pub(crate) fn touch_started(&self, pointer_id: i32) -> TouchId {
        let mut state = self.state.borrow_mut();
        let id = TouchId(state.next_touch_id);
        state.next_touch_id += 1;
        state.touches.insert(pointer_id, id);
        id
    }

    pub(crate) fn touch(&self, pointer_id: i32) -> Option<TouchId> {
        self.state.borrow().touches.get(&pointer_id).copied()
    }

    pub(crate) fn touch_ended(&self, pointer_id: i32) -> Option<TouchId> {
        self.state.borrow_mut().touches.remove(&pointer_id)
    }

    pub(crate) fn set_modifiers(&self, modifiers: Modifiers, capslock: Capslock) -> bool {
        let mut state = self.state.borrow_mut();
        let changed = state.modifiers != modifiers || state.capslock != capslock;
        state.modifiers = modifiers;
        state.capslock = capslock;
        changed
    }

    pub(crate) fn set_mouse_position(&self, position: Point<Pixels>) {
        self.state.borrow_mut().mouse_position = position;
    }

    pub(crate) fn pressed_button(&self) -> Option<MouseButton> {
        self.state.borrow().pressed_button
    }

    pub(crate) fn set_pressed_button(&self, button: Option<MouseButton>) {
        self.state.borrow_mut().pressed_button = button;
    }

    /// The click count of a press of `button` at `position` now.
    pub(crate) fn click_count(&self, button: MouseButton, position: Point<Pixels>) -> usize {
        let now = Instant::now();
        let mut state = self.state.borrow_mut();
        let count = match &state.last_click {
            Some(last)
                if last.button == button
                    && now.duration_since(last.at) < MULTI_CLICK_INTERVAL
                    && f32::from((last.position.x - position.x).abs()) < MULTI_CLICK_SLOP
                    && f32::from((last.position.y - position.y).abs()) < MULTI_CLICK_SLOP =>
            {
                last.count + 1
            }
            _ => 1,
        };
        state.last_click = Some(LastClick {
            button,
            at: now,
            position,
            count,
        });
        count
    }

    pub(crate) fn last_click_count(&self) -> usize {
        self.state
            .borrow()
            .last_click
            .as_ref()
            .map_or(1, |click| click.count)
    }

    pub(crate) fn set_hovered(&self, hovered: bool) {
        if std::mem::replace(&mut self.state.borrow_mut().hovered, hovered) == hovered {
            return;
        }
        let callback = self.callbacks.borrow_mut().hover_status_change.take();
        if let Some(mut callback) = callback {
            callback(hovered);
            self.callbacks.borrow_mut().hover_status_change = Some(callback);
        }
    }

    /// The system back button or gesture; true when the application took it.
    pub(crate) fn handle_back(&self) -> bool {
        if !self.state.borrow().back_enabled {
            return false;
        }
        let callback = self.callbacks.borrow_mut().back.take();
        match callback {
            Some(mut callback) => {
                callback();
                self.callbacks.borrow_mut().back = Some(callback);
                true
            }
            None => false,
        }
    }
}

fn logical_size(state: &WindowState) -> Size<Pixels> {
    size(
        px(state.size.width.0 as f32 / state.scale_factor),
        px(state.size.height.0 as f32 / state.scale_factor),
    )
}

/// What GPUI holds: the `PlatformWindow` face of the activity's window.
pub struct AndroidPlatformWindow(pub(crate) Rc<AndroidWindow>);

impl HasWindowHandle for AndroidPlatformWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        let state = self.0.state.borrow();
        let native_window = state
            .native_window
            .as_ref()
            .ok_or(HandleError::Unavailable)?;
        let raw =
            RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(native_window.ptr().cast()));
        // SAFETY: the surface outlives the borrow for all GPUI does with it
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for AndroidPlatformWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        // SAFETY: the Android display handle carries no pointer
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Android(
                AndroidDisplayHandle::new(),
            ))
        })
    }
}

impl PlatformWindow for AndroidPlatformWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: Point::default(),
            size: logical_size(&self.0.state.borrow()),
        }
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn window_bounds(&self) -> WindowBounds {
        // the system places and sizes the window
        WindowBounds::Fullscreen(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        logical_size(&self.0.state.borrow())
    }

    fn resize(&mut self, _size: Size<Pixels>) {}

    fn scale_factor(&self) -> f32 {
        self.0.state.borrow().scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        self.0.state.borrow().appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(self.0.display()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.0.state.borrow().mouse_position
    }

    fn modifiers(&self) -> Modifiers {
        self.0.state.borrow().modifiers
    }

    fn capslock(&self) -> Capslock {
        self.0.state.borrow().capslock
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0.state.borrow_mut().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.state.borrow_mut().input_handler.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        // GPUI draws its own prompt
        None
    }

    fn activate(&self) {}

    fn is_active(&self) -> bool {
        self.0.state.borrow().active
    }

    fn visibility(&self) -> WindowVisibility {
        if self.0.has_surface() {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        }
    }

    fn is_hovered(&self) -> bool {
        self.0.state.borrow().hovered
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.0.state.borrow_mut().title = title.to_owned();
    }

    fn get_title(&self) -> String {
        self.0.state.borrow().title.clone()
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {}

    fn minimize(&self) {}

    fn zoom(&self) {}

    fn toggle_fullscreen(&self) {}

    fn is_fullscreen(&self) -> bool {
        false
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        // weak: the waker lives in the window's invalidator, which the frame
        // callback stored in this window captures
        let window = Rc::downgrade(&self.0);
        Some(Rc::new(move || {
            if let Some(window) = window.upgrade() {
                window.request_frame();
            }
        }))
    }

    fn schedule_frame(&self) {
        self.0.request_frame();
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.callbacks.borrow_mut().request_frame = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.callbacks.borrow_mut().input = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().active_status_change = Some(callback);
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.0.callbacks.borrow_mut().visibility_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().hover_status_change = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.callbacks.borrow_mut().resize = Some(callback);
    }

    fn on_moved(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.callbacks.borrow_mut().should_close = Some(callback);
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        let mut state = self.0.state.borrow_mut();
        if state.native_window.is_some() {
            state.renderer.draw(scene);
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.state.borrow().renderer.sprite_atlas().clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        // phone and tablet panels rotate and come in PenTile layouts, where
        // LCD subpixel text shows colour fringes
        false
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.state.borrow().renderer.gpu_specs()
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn window_controls(&self) -> WindowControls {
        WindowControls {
            fullscreen: false,
            maximize: false,
            minimize: false,
            window_menu: false,
        }
    }

    fn insets(&self) -> WindowInsets {
        self.0.state.borrow().insets.clone()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.0.callbacks.borrow_mut().insets_changed = Some(callback);
    }

    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().back = Some(callback);
    }

    fn set_back_enabled(&self, enabled: bool) {
        self.0.state.borrow_mut().back_enabled = enabled;
    }

    fn show_soft_keyboard(&self) {
        super::activity_events::show_soft_keyboard(true);
    }

    fn hide_soft_keyboard(&self) {
        super::activity_events::show_soft_keyboard(false);
    }

    fn text_input_state_changed(&self, change: gpui::TextInputStateChange) {
        // The keyboard closes with the text input; it opens when one is
        // tapped (`show_keyboard_for_tap`), not for every focus change.
        match change {
            gpui::TextInputStateChange::FocusGained => {}
            gpui::TextInputStateChange::FocusLost => {
                super::activity_events::show_soft_keyboard(false)
            }
            gpui::TextInputStateChange::SelectionChanged
            | gpui::TextInputStateChange::ContentChanged => {}
        }
    }
}
