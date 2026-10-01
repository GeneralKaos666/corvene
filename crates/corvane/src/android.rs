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
    gpui_android::set_soft_keyboard_handler(show_keyboard);
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
/// * `files/git/bin`: the bundled git ([`bundled_git`])
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
    let git = bundled_git(&files);
    // SAFETY: nothing else in the process reads the environment yet: this
    // runs first on the native thread, before any Rust thread is spawned
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CACHE_HOME", &cache);
        std::env::set_var("TMPDIR", &tmp);
        if let Some(git) = git {
            // `corvane_git::find_git` and git itself (ssh, git-lfs)
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut dirs = vec![git.bin.clone()];
            dirs.extend(std::env::split_paths(&path));
            if let Ok(path) = std::env::join_paths(dirs) {
                std::env::set_var("PATH", path);
            }
            std::env::set_var("CORVANE_GIT", git.bin.join("git"));
            std::env::set_var("GIT_EXEC_PATH", &git.bin);
            std::env::set_var("GIT_TEMPLATE_DIR", &git.templates);
            // there is no /etc/gitconfig
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            // The system's trusted certificates. Android names the files by
            // OpenSSL's old subject hash, which OpenSSL 3 does not look up,
            // so the directory alone verifies nothing: the same
            // certificates are also handed over as one bundle.
            std::env::set_var("GIT_SSL_CAPATH", "/system/etc/security/cacerts");
            if let Some(bundle) = &git.ca_bundle {
                std::env::set_var("GIT_SSL_CAINFO", bundle);
            }
        }
    }
}

struct BundledGit {
    /// `git`, `git-remote-https`, `ssh`, `git-lfs`, …: `GIT_EXEC_PATH`.
    bin: PathBuf,
    /// An empty `GIT_TEMPLATE_DIR` (git warns about a missing one).
    templates: PathBuf,
    /// The system's certificate store as one PEM file.
    ca_bundle: Option<PathBuf>,
}

/// The executables the package carries as `lib*.so` (the only files an app
/// may execute are those the installer extracts into its native library
/// directory; `packaging/android/git/build.sh`), under the names git looks
/// for: symbolic links in `files/git/bin`, made again on every start because
/// the library directory moves with each update.
fn bundled_git(files: &Path) -> Option<BundledGit> {
    const LINKS: &[(&str, &str)] = &[
        ("git", "libgit.so"),
        ("git-remote-https", "libgit-remote-https.so"),
        ("git-remote-http", "libgit-remote-https.so"),
        ("ssh", "libssh.so"),
        ("git-lfs", "libgit-lfs.so"),
        ("git-sh-setup", "libgit-sh-setup.so"),
        ("git-sh-i18n", "libgit-sh-i18n.so"),
        ("git-submodule", "libgit-submodule.so"),
        ("git-mergetool", "libgit-mergetool.so"),
        ("git-mergetool--lib", "libgit-mergetool--lib.so"),
    ];
    let libraries = native_library_dir()?;
    if !libraries.join("libgit.so").exists() {
        return None;
    }
    let bin = files.join("git/bin");
    let templates = files.join("git/templates");
    let _ = std::fs::remove_dir_all(&bin);
    for dir in [&bin, &templates] {
        std::fs::create_dir_all(dir).ok()?;
    }
    for (name, library) in LINKS {
        let target = libraries.join(library);
        if target.exists() {
            let _ = std::os::unix::fs::symlink(target, bin.join(name));
        }
    }
    let ca_bundle = ca_bundle(&files.join("git/cacert.pem"));
    Some(BundledGit {
        bin,
        templates,
        ca_bundle,
    })
}

/// Writes the certificates Android trusts (the Conscrypt module's store,
/// which replaced `/system/etc/security/cacerts` in Android 14, and the ones
/// the user installed) into `bundle`, on every start so removals and
/// additions are followed.
fn ca_bundle(bundle: &Path) -> Option<PathBuf> {
    let system = [
        "/apex/com.android.conscrypt/cacerts",
        "/system/etc/security/cacerts",
    ]
    .into_iter()
    .find(|dir| Path::new(dir).is_dir())?;
    let mut pem = Vec::new();
    for dir in [system, "/data/misc/user/0/cacerts-added"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(certificate) = std::fs::read(entry.path()) {
                pem.extend_from_slice(&certificate);
                pem.push(b'\n');
            }
        }
    }
    if pem.is_empty() {
        return None;
    }
    std::fs::write(bundle, pem).ok()?;
    Some(bundle.to_path_buf())
}

/// Where the installer extracted the package's native libraries: the
/// directory this library was loaded from.
fn native_library_dir() -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: `dladdr` fills `info` for an address inside this library and
    // `dli_fname` then points at the loader's own NUL-terminated path
    let path = unsafe {
        if libc::dladdr(native_library_dir as *const libc::c_void, info.as_mut_ptr()) == 0 {
            return None;
        }
        let name = info.assume_init().dli_fname;
        if name.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(name)
            .to_string_lossy()
            .into_owned()
    };
    Path::new(&path).parent().map(Path::to_path_buf)
}

// ── CorvaneActivity (packaging/android) ─────────────────────────────────────
//
// The activity's input view forwards what the keyboard does; these run on
// the Java UI thread and only queue the event for the native thread.

use std::ffi::c_void;

use gpui_android::ActivityEvent;

/// `CorvaneActivity.showKeyboard(boolean)`.
fn show_keyboard(show: bool) {
    let result = gpui_android::jni::with_env(|env| {
        let class =
            gpui_android::jni::find_app_class(env, "com.wasimaster.corvane.CorvaneActivity")?;
        env.call_static_method(
            &class,
            jni::jni_str!("showKeyboard"),
            jni::jni_sig!("(Z)V"),
            &[jni::objects::JValue::Bool(show)],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    });
    if let Err(err) = result {
        tracing::warn!("showKeyboard failed: {err}");
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeCommitText(
    _env: *mut c_void,
    _class: *mut c_void,
    text: *mut c_void,
) {
    gpui_android::post(ActivityEvent::CommitText(
        gpui_android::jni::string_from_raw(text),
    ));
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeSetComposingText(
    _env: *mut c_void,
    _class: *mut c_void,
    text: *mut c_void,
) {
    gpui_android::post(ActivityEvent::SetComposingText(
        gpui_android::jni::string_from_raw(text),
    ));
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeFinishComposing(
    _env: *mut c_void,
    _class: *mut c_void,
) {
    gpui_android::post(ActivityEvent::FinishComposing);
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeDeleteSurrounding(
    _env: *mut c_void,
    _class: *mut c_void,
    before: i32,
    after: i32,
) {
    gpui_android::post(ActivityEvent::DeleteSurrounding {
        before: before.max(0) as usize,
        after: after.max(0) as usize,
    });
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeKey(
    _env: *mut c_void,
    _class: *mut c_void,
    key_code: i32,
    down: u8,
    meta_state: i32,
    unicode: i32,
) {
    gpui_android::post(ActivityEvent::Key {
        key_code,
        down: down != 0,
        meta_state,
        unicode: unicode.max(0) as u32,
    });
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeOpenUrl(
    _env: *mut c_void,
    _class: *mut c_void,
    url: *mut c_void,
) {
    gpui_android::post(ActivityEvent::OpenUrl(gpui_android::jni::string_from_raw(
        url,
    )));
}
