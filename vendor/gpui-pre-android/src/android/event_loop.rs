//! The event loop `Platform::run` blocks in.
//!
//! `android-activity` calls `android_main` on a thread of its own and feeds
//! it the activity's lifecycle and input through `AndroidApp::poll_events`.
//! The loop sleeps in that call until the looper has something: a lifecycle
//! command, input, a foreground task, or the next frame of a window that
//! asked for one. An idle application does not wake up at all.
//!
//! Ported from the event loop in gpui-mobile's `src/android/jni.rs`, which
//! polled without blocking, slept 500 µs and rendered on every iteration.

use std::time::{Duration, Instant};

use android_activity::{
    input::{
        Axis, Button, InputEvent, KeyAction, KeyEvent, KeyMapChar, Keycode, MetaState,
        MotionAction, MotionEvent, ToolType,
    },
    InputStatus, MainEvent, PollEvent,
};
use gpui::{
    point, px, AppLifecyclePhase, Capslock, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, PlatformInput, ScrollDelta, ScrollWheelEvent, TouchEvent, TouchPhase,
};

use super::{
    dispatcher, keyboard::android_key_to_keystroke, platform::AndroidPlatform,
    window::AndroidWindow,
};

/// Frames are paced to this interval; presentation itself waits for vsync.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// What one mouse wheel notch scrolls, in logical pixels
/// (`ViewConfiguration.getScaledVerticalScrollFactor()` is 64 dp).
const WHEEL_NOTCH: f32 = 64.0;

/// Lifecycle commands noted inside `poll_events`, handled after it returns:
/// the handlers call back into `AndroidApp`, which is locked during the poll
/// callback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    InitWindow,
    SurfaceChanged,
    Redraw,
    GainedFocus,
    LostFocus,
    ConfigChanged,
    LowMemory,
    Start,
    Resume,
    Pause,
    Stop,
    Destroy,
}

impl AndroidPlatform {
    pub(crate) fn run_event_loop(&self) {
        let app = self.app.clone();
        let mut commands = Vec::new();
        let mut last_frame: Option<Instant> = None;

        while !self.should_quit.get() {
            // run what is already queued before deciding how long to sleep
            self.run_foreground_tasks();
            if self.should_quit.get() {
                break;
            }

            let timeout = match self.window() {
                Some(window) if window.wants_frame() => {
                    Some(last_frame.map_or(Duration::ZERO, |last| {
                        FRAME_INTERVAL.saturating_sub(last.elapsed())
                    }))
                }
                _ => None,
            };
            app.poll_events(timeout, |event| {
                if let PollEvent::Main(event) = event {
                    match event {
                        // the surface dies when this callback returns
                        MainEvent::TerminateWindow { .. } => {
                            commands.retain(|command| *command != Command::InitWindow);
                            if let Some(window) = self.window() {
                                window.surface_destroyed();
                            }
                        }
                        MainEvent::InitWindow { .. } => commands.push(Command::InitWindow),
                        MainEvent::WindowResized { .. }
                        | MainEvent::ContentRectChanged { .. }
                        | MainEvent::InsetsChanged { .. } => commands.push(Command::SurfaceChanged),
                        MainEvent::RedrawNeeded { .. } => commands.push(Command::Redraw),
                        MainEvent::GainedFocus => commands.push(Command::GainedFocus),
                        MainEvent::LostFocus => commands.push(Command::LostFocus),
                        MainEvent::ConfigChanged { .. } => commands.push(Command::ConfigChanged),
                        MainEvent::LowMemory => commands.push(Command::LowMemory),
                        MainEvent::Start => commands.push(Command::Start),
                        MainEvent::Resume { .. } => commands.push(Command::Resume),
                        MainEvent::Pause => commands.push(Command::Pause),
                        MainEvent::Stop => commands.push(Command::Stop),
                        MainEvent::Destroy => commands.push(Command::Destroy),
                        _ => {}
                    }
                }
            });

            for command in commands.drain(..) {
                self.handle_command(command);
            }
            self.process_input();
            self.run_foreground_tasks();

            if let Some(window) = self.window() {
                let due = last_frame.is_none_or(|last| last.elapsed() >= FRAME_INTERVAL);
                if window.wants_frame() && due {
                    last_frame = Some(Instant::now());
                    window.frame();
                }
            }
        }

        log::info!("the event loop is ending");
        let quit = self.callbacks.borrow_mut().quit.take();
        if let Some(mut quit) = quit {
            quit();
        }
        if let Some(window) = self.window.borrow_mut().take() {
            window.close();
        }
    }

    fn run_foreground_tasks(&self) {
        let receiver = self.main_receiver.clone();
        for runnable in receiver.try_iter().flatten() {
            dispatcher::run(runnable);
        }
    }

    fn handle_command(&self, command: Command) {
        log::debug!("lifecycle: {command:?}");
        match command {
            Command::InitWindow => self.init_window(),
            Command::SurfaceChanged => {
                if let Some(window) = self.window() {
                    window.surface_changed(self.scale_factor());
                }
            }
            Command::Redraw => {
                if let Some(window) = self.window() {
                    window.surface_changed(self.scale_factor());
                    window.redraw();
                }
            }
            Command::GainedFocus => {
                if let Some(window) = self.window() {
                    window.set_active(true);
                }
            }
            Command::LostFocus => {
                if let Some(window) = self.window() {
                    window.set_active(false);
                }
            }
            Command::ConfigChanged => {
                if let Some(window) = self.window() {
                    window.surface_changed(self.scale_factor());
                    window.set_appearance(self.appearance());
                }
                self.fire_keyboard_layout_change();
            }
            Command::LowMemory => self.fire_memory_warning(),
            Command::Start => self.fire_lifecycle(AppLifecyclePhase::Foreground),
            Command::Resume => self.fire_lifecycle(AppLifecyclePhase::Active),
            Command::Pause => self.fire_lifecycle(AppLifecyclePhase::Inactive),
            Command::Stop => self.fire_lifecycle(AppLifecyclePhase::Background),
            Command::Destroy => self.should_quit.set(true),
        }
    }

    /// A surface is ready: the first one creates the window and lets the
    /// application finish launching, later ones are handed to the window.
    fn init_window(&self) {
        let Some(native_window) = self.app.native_window() else {
            return;
        };
        let scale_factor = self.scale_factor();
        if let Some(window) = self.window() {
            window.surface_created(native_window, scale_factor);
            return;
        }
        match AndroidWindow::new(
            self.app.clone(),
            native_window,
            self.gpu_context(),
            scale_factor,
            self.appearance(),
        ) {
            Ok(window) => *self.window.borrow_mut() = Some(window),
            Err(err) => {
                log::error!("failed to create the window: {err:#}");
                self.should_quit.set(true);
                return;
            }
        }
        let finish_launching = self.finish_launching.borrow_mut().take();
        if let Some(finish_launching) = finish_launching {
            finish_launching();
        }
    }

    fn process_input(&self) {
        let Some(window) = self.window() else {
            return;
        };
        let mut iter = match self.app.input_events_iter() {
            Ok(iter) => iter,
            Err(err) => {
                log::error!("no input events: {err:?}");
                return;
            }
        };
        while iter.next(|event| match event {
            InputEvent::MotionEvent(motion) => {
                handle_motion(&window, motion);
                InputStatus::Handled
            }
            InputEvent::KeyEvent(key) => {
                if self.handle_key(&window, key) {
                    InputStatus::Handled
                } else {
                    InputStatus::Unhandled
                }
            }
            _ => InputStatus::Unhandled,
        }) {}
    }

    /// True when the key was used, so the system must not apply its default
    /// (volume, back, …).
    fn handle_key(&self, window: &AndroidWindow, event: &KeyEvent) -> bool {
        let meta = event.meta_state();
        let modifiers = modifiers(meta);
        let capslock = Capslock {
            on: meta.caps_lock_on(),
        };
        if window.set_modifiers(modifiers, capslock) {
            window.handle_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));
        }

        let key_code = event.key_code();
        let down = match event.action() {
            KeyAction::Down => true,
            KeyAction::Up => false,
            _ => return false,
        };

        // Back acts on release, like `Activity.onBackPressed`: a handler the
        // application enabled, else Escape (close the menu, dialog or
        // foldout), else the system's default, which needs the press
        // unhandled too.
        if key_code == Keycode::Back {
            if down {
                return false;
            }
            if window.handle_back() {
                return true;
            }
            let escape = Keystroke {
                modifiers: Modifiers::default(),
                key: "escape".into(),
                key_char: None,
            };
            let handled = window.handle_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke: escape.clone(),
                is_held: false,
                prefer_character_input: false,
            }));
            window.handle_input(PlatformInput::KeyUp(KeyUpEvent { keystroke: escape }));
            return handled;
        }

        let unicode = match self
            .app
            .device_key_character_map(event.device_id())
            .and_then(|map| map.get(key_code, meta))
        {
            Ok(KeyMapChar::Unicode(c)) => c as u32,
            _ => 0,
        };
        let code: u32 = key_code.into();
        let Some(keystroke) = android_key_to_keystroke(code as i32, modifiers, unicode) else {
            return false;
        };
        if down {
            window.handle_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: event.repeat_count() > 0,
                prefer_character_input: false,
            }))
        } else {
            window.handle_input(PlatformInput::KeyUp(KeyUpEvent { keystroke }))
        }
    }
}

fn modifiers(meta: MetaState) -> Modifiers {
    Modifiers {
        control: meta.ctrl_on(),
        alt: meta.alt_on(),
        shift: meta.shift_on(),
        platform: meta.meta_on(),
        function: meta.function_on(),
    }
}

fn handle_motion(window: &AndroidWindow, motion: &MotionEvent) {
    let action = motion.action();
    let action_pointer = match action {
        MotionAction::PointerDown | MotionAction::PointerUp => motion.pointer_index(),
        _ => 0,
    };
    if action_pointer >= motion.pointer_count() {
        return;
    }
    match motion.pointer_at_index(action_pointer).tool_type() {
        ToolType::Mouse => handle_mouse(window, motion, action),
        _ => handle_touch(window, motion, action, action_pointer),
    }
}

/// Fingers and styluses: raw touches, which GPUI's gesture recognizers turn
/// into taps, long presses, pans and flings.
fn handle_touch(
    window: &AndroidWindow,
    motion: &MotionEvent,
    action: MotionAction,
    action_pointer: usize,
) {
    let touch = |id, phase, index: usize| {
        let pointer = motion.pointer_at_index(index);
        PlatformInput::Touch(TouchEvent {
            id,
            phase,
            position: window.logical_point(pointer.x(), pointer.y()),
            predicted_position: None,
            force: Some(pointer.pressure().clamp(0.0, 1.0)),
        })
    };
    match action {
        MotionAction::Down | MotionAction::PointerDown => {
            let pointer_id = motion.pointer_at_index(action_pointer).pointer_id();
            let id = window.touch_started(pointer_id);
            window.handle_input(touch(id, TouchPhase::Started, action_pointer));
        }
        MotionAction::Move => {
            for index in 0..motion.pointer_count() {
                let pointer_id = motion.pointer_at_index(index).pointer_id();
                if let Some(id) = window.touch(pointer_id) {
                    window.handle_input(touch(id, TouchPhase::Moved, index));
                }
            }
        }
        MotionAction::Up | MotionAction::PointerUp => {
            let pointer_id = motion.pointer_at_index(action_pointer).pointer_id();
            if let Some(id) = window.touch_ended(pointer_id) {
                window.handle_input(touch(id, TouchPhase::Ended, action_pointer));
            }
        }
        MotionAction::Cancel => {
            for index in 0..motion.pointer_count() {
                let pointer_id = motion.pointer_at_index(index).pointer_id();
                if let Some(id) = window.touch_ended(pointer_id) {
                    window.handle_input(touch(id, TouchPhase::Cancelled, index));
                }
            }
        }
        _ => {}
    }
}

fn mouse_button(button: Button) -> Option<MouseButton> {
    match button {
        Button::Primary => Some(MouseButton::Left),
        Button::Secondary => Some(MouseButton::Right),
        Button::Tertiary => Some(MouseButton::Middle),
        Button::Back => Some(MouseButton::Navigate(gpui::NavigationDirection::Back)),
        Button::Forward => Some(MouseButton::Navigate(gpui::NavigationDirection::Forward)),
        _ => None,
    }
}

/// Mice and touchpads (Chromebooks, DeX, a mouse on a tablet): hover, the
/// buttons and the wheel, as on the desktop.
fn handle_mouse(window: &AndroidWindow, motion: &MotionEvent, action: MotionAction) {
    let pointer = motion.pointer_at_index(0);
    let position = window.logical_point(pointer.x(), pointer.y());
    let modifiers = modifiers(motion.meta_state());
    window.set_mouse_position(position);

    match action {
        MotionAction::HoverEnter | MotionAction::HoverMove | MotionAction::Move => {
            window.set_hovered(true);
            window.handle_input(PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: window.pressed_button(),
                modifiers,
            }));
        }
        MotionAction::HoverExit => {
            window.set_hovered(false);
            window.handle_input(PlatformInput::MouseExited(MouseExitEvent {
                position,
                pressed_button: window.pressed_button(),
                modifiers,
            }));
        }
        // a press is ACTION_DOWN followed by ACTION_BUTTON_PRESS naming the
        // button; the latter is the event to act on
        MotionAction::ButtonPress => {
            let Some(button) = mouse_button(motion.action_button()) else {
                return;
            };
            window.set_pressed_button(Some(button));
            let click_count = window.click_count(button, position);
            window.handle_input(PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count,
                first_mouse: false,
            }));
        }
        MotionAction::ButtonRelease => {
            let Some(button) = mouse_button(motion.action_button()) else {
                return;
            };
            window.set_pressed_button(None);
            window.handle_input(PlatformInput::MouseUp(MouseUpEvent {
                button,
                position,
                modifiers,
                click_count: window.last_click_count(),
            }));
        }
        MotionAction::Cancel => {
            if let Some(button) = window.pressed_button() {
                window.set_pressed_button(None);
                window.handle_input(PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers,
                    click_count: 1,
                }));
            }
        }
        MotionAction::Scroll => {
            // positive VSCROLL scrolls up and positive HSCROLL right; GPUI's
            // delta is how far the content moves
            let vertical = pointer.axis_value(Axis::Vscroll);
            let horizontal = pointer.axis_value(Axis::Hscroll);
            window.handle_input(PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta: ScrollDelta::Pixels(point(
                    px(-horizontal * WHEEL_NOTCH),
                    px(vertical * WHEEL_NOTCH),
                )),
                modifiers,
                touch_phase: TouchPhase::Moved,
            }));
        }
        _ => {}
    }
}
