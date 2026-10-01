//! The Android platform layer for gpui-pre 0.3.7.
//!
//! A port of the `src/android` module of gpui-mobile
//! (<https://github.com/itsbalamurali/gpui-mobile>, Copyright (c) Balamurali
//! Pandranki, used under its Apache-2.0 option; see `LICENSE-APACHE`).
//! gpui-mobile targets Zed's GPUI of April 2026; each module's doc comment
//! says what changed for the GPUI pinned here. Its mouse emulation, momentum
//! scroller, platform views and device packages are not part of this crate:
//! the pinned GPUI recognizes touch gestures itself.
//!
//! ```text
//! android_main(app)                      the application's entry point
//!   └── Application::with_platform(current_platform(app)).run(…)
//!         └── AndroidPlatform::run       blocks in the event loop
//! ```

#![cfg(target_os = "android")]
// GPUI's callback signatures, as in its own platform crates
#![allow(clippy::type_complexity)]

mod android {
    pub mod activity_events;
    pub(crate) mod dispatcher;
    pub(crate) mod display;
    pub(crate) mod event_loop;
    pub mod jni;
    pub mod keyboard;
    pub(crate) mod platform;
    pub(crate) mod window;

    /// Routes `log` to logcat; later calls do nothing.
    pub fn init_logger() {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("gpui"),
        );
    }
}

use std::rc::Rc;

pub use android::activity_events::{post, set_soft_keyboard_handler, ActivityEvent};
pub use android::display::AndroidDisplay;
pub use android::init_logger;
pub use android::jni;
pub use android::keyboard;
pub use android::platform::AndroidPlatform;
pub use android::window::AndroidPlatformWindow;
pub use android_activity::{self, AndroidApp};

/// The platform for the activity `android-activity` passed to
/// `android_main`. Call it on that thread.
pub fn current_platform(app: AndroidApp) -> Rc<AndroidPlatform> {
    Rc::new(AndroidPlatform::new(app))
}
