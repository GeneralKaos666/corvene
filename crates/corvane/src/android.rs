//! Android entry point. The activity (`packaging/android`) is a
//! `NativeActivity`: it loads this library, `libcorvane.so`, and
//! `android-activity` calls `android_main` on a thread of its own, which then
//! runs the same `main` as the desktop binary. Empty on other platforms.

#![cfg(target_os = "android")]

#[path = "main.rs"]
mod app;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gpui_android::AndroidApp;

static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

/// The activity `android_main` was started for.
pub(crate) fn android_app() -> AndroidApp {
    ANDROID_APP
        .get()
        .cloned()
        .expect("android_main stores the AndroidApp before main runs")
}

#[unsafe(no_mangle)]
fn android_main(android_app: AndroidApp) {
    prepare_environment(&android_app);
    let _ = ANDROID_APP.set(android_app);
    std::panic::set_hook(Box::new(|info| {
        // stderr goes nowhere; `logging` sends tracing to logcat
        tracing::error!("panic: {info}");
    }));
    app::main();
    // The activity is gone. A later launch would call `android_main` again
    // in this process, with the store still locked and GPUI's globals set.
    std::process::exit(0);
}

/// An Android process starts with no `HOME` and an unwritable temporary
/// directory. Everything Corvane and git keep lives in the app-private
/// storage (`/data/user/0/<package>`), a full Linux filesystem:
///
/// * `files/home`: `HOME` (git's global config, `~/.ssh`, the XDG data and
///   state directories)
/// * `cache`: `XDG_CACHE_HOME`, which the system may clear
/// * `cache/tmp`: `TMPDIR`
fn prepare_environment(android_app: &AndroidApp) {
    let files = android_app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/corvane"));
    let home = files.join("home");
    let cache = files
        .parent()
        .unwrap_or(Path::new("/data/local/tmp"))
        .join("cache");
    let tmp = cache.join("tmp");
    for dir in [&home, &tmp] {
        let _ = std::fs::create_dir_all(dir);
    }
    // SAFETY: nothing else in the process reads the environment yet: this
    // runs first on the native thread, before any Rust thread is spawned
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CACHE_HOME", &cache);
        std::env::set_var("TMPDIR", &tmp);
    }
}
