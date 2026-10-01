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
    let files = android_app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/corvane"));
    prepare_environment(&files);
    let _ = ANDROID_APP.set(android_app);
    gpui_android::set_soft_keyboard_handler(show_keyboard);
    gpui_android::set_path_prompt_handler(pick_folder);
    gpui_android::set_url_handler(open_url);
    gpui_android::set_path_opener(open_path);
    corvane_platform::android::set_bridge(Box::new(ActivityBridge));
    std::panic::set_hook(Box::new(|info| {
        // stderr goes nowhere; `logging` sends tracing to logcat
        tracing::error!("panic: {info}");
    }));
    serve_askpass();
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
fn prepare_environment(files: &Path) {
    let home = files.join("home");
    let cache = files
        .parent()
        .unwrap_or(Path::new("/data/local/tmp"))
        .join("cache");
    let tmp = cache.join("tmp");
    for dir in [&home, &tmp] {
        let _ = std::fs::create_dir_all(dir);
    }
    let git = bundled_git(files);
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
            // credentials: git runs the helper, which asks `serve_askpass`
            std::env::set_var("CORVANE_ASKPASS_PROGRAM", git.bin.join("corvane-askpass"));
            std::env::set_var("CORVANE_ASKPASS_SOCKET", cache.join("askpass.sock"));
            std::env::set_var("GIT_EXEC_PATH", &git.bin);
            std::env::set_var("GIT_TEMPLATE_DIR", &git.templates);
            // No terminal to ask whether an unknown host's key is right:
            // the first key seen is kept in ~/.ssh/known_hosts and a
            // changed one is still refused.
            if std::env::var_os("GIT_SSH_COMMAND").is_none() {
                std::env::set_var(
                    "GIT_SSH_COMMAND",
                    format!(
                        "{} -o StrictHostKeyChecking=accept-new",
                        git.bin.join("ssh").display()
                    ),
                );
            }
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
        ("ssh-keygen", "libssh-keygen.so"),
        ("git-lfs", "libgit-lfs.so"),
        ("corvane-askpass", "libcorvane-askpass.so"),
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

const ACTIVITY: &str = "com.wasimaster.corvane.CorvaneActivity";

/// Calls a static method of `CorvaneActivity` that returns `boolean` or
/// `void`: `Some(Some(result))`, `Some(None)` for `void`, and `None` (with a
/// log line) when the call fails.
macro_rules! activity_call {
    ($name:literal, $sig:literal, $args:expr) => {{
        let result = gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            env.call_static_method(&class, jni::jni_str!($name), jni::jni_sig!($sig), $args)
                // a `boolean` result; `None` for `void`
                .map(|value| value.z().ok())
                .map_err(|err| err.to_string())
        });
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!("CorvaneActivity.{} failed: {err}", $name);
                None
            }
        }
    }};
}

/// A static `int` method of `CorvaneActivity` without arguments.
macro_rules! activity_int {
    ($name:literal) => {{
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            env.call_static_method(&class, jni::jni_str!($name), jni::jni_sig!("()I"), &[])
                .and_then(|value| value.i())
                .map_err(|err| err.to_string())
        })
        .ok()
    }};
}

/// A static `void` method of `CorvaneActivity` taking strings; true when the
/// call went through.
macro_rules! activity_strings {
    ($name:literal, $sig:literal, [$($arg:expr),+]) => {{
        let result = gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let strings = [$(env.new_string($arg).map_err(|err| err.to_string())?),+];
            let args: Vec<jni::objects::JValue> =
                strings.iter().map(|s| jni::objects::JValue::Object(s)).collect();
            env.call_static_method(&class, jni::jni_str!($name), jni::jni_sig!($sig), &args)
                .map(|_| ())
                .map_err(|err| err.to_string())
        });
        if let Err(err) = &result {
            tracing::warn!("CorvaneActivity.{} failed: {err}", $name);
        }
        result.is_ok()
    }};
}

/// A static method of `CorvaneActivity` that takes one string and returns an
/// error message, empty for success.
macro_rules! activity_result {
    ($name:literal, $arg:expr) => {{
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let arg = env.new_string($arg).map_err(|err| err.to_string())?;
            let message = env
                .call_static_method(
                    &class,
                    jni::jni_str!($name),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[jni::objects::JValue::Object(&arg)],
                )
                .and_then(|value| value.l())
                .map_err(|err| err.to_string())?;
            Ok(gpui_android::jni::get_string(env, &message))
        })
        .and_then(|message| {
            if message.is_empty() {
                Ok(())
            } else {
                Err(message)
            }
        })
    }};
}

/// `CorvaneActivity.showKeyboard(boolean)`.
fn show_keyboard(show: bool) {
    activity_call!("showKeyboard", "(Z)V", &[jni::objects::JValue::Bool(show)]);
}

struct ActivityBridge;

impl corvane_platform::android::Bridge for ActivityBridge {
    fn has_all_files_access(&self) -> bool {
        activity_call!("hasAllFilesAccess", "()Z", &[])
            .flatten()
            .unwrap_or(false)
    }

    fn can_request_all_files_access(&self) -> bool {
        activity_call!("canRequestAllFilesAccess", "()Z", &[])
            .flatten()
            .unwrap_or(false)
    }

    fn allows_downloaded_code(&self) -> bool {
        activity_call!("allowsDownloadedCode", "()Z", &[])
            .flatten()
            .unwrap_or(false)
    }

    fn grammar_module_dir(&self) -> Option<PathBuf> {
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let dir = env
                .call_static_method(
                    &class,
                    jni::jni_str!("grammarModuleDir"),
                    jni::jni_sig!("()Ljava/lang/String;"),
                    &[],
                )
                .and_then(|value| value.l())
                .map_err(|err| err.to_string())?;
            Ok(gpui_android::jni::get_string(env, &dir))
        })
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
    }

    fn install_grammar_module(&self) {
        activity_call!("installGrammarModule", "()V", &[]);
    }

    fn uninstall_grammar_module(&self) {
        activity_call!("uninstallGrammarModule", "()V", &[]);
    }

    fn request_all_files_access(&self) {
        activity_call!("requestAllFilesAccess", "()V", &[]);
    }

    fn notifications_allowed(&self) -> Option<bool> {
        // 0: not asked yet, 1: allowed, 2: denied
        match activity_int!("notificationPermission") {
            Some(1) => Some(true),
            Some(2) => Some(false),
            _ => None,
        }
    }

    fn request_notification_permission(&self) {
        activity_call!("requestNotificationPermission", "()V", &[]);
    }

    fn show_notification(&self, identifier: &str, title: &str, body: &str, payload: &str) {
        let _ = activity_strings!(
            "showNotification",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            [identifier, title, body, payload]
        );
    }

    fn package_installed(&self, package: &str) -> bool {
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let package = env.new_string(package).map_err(|err| err.to_string())?;
            env.call_static_method(
                &class,
                jni::jni_str!("packageInstalled"),
                jni::jni_sig!("(Ljava/lang/String;)Z"),
                &[jni::objects::JValue::Object(&package)],
            )
            .and_then(|value| value.z())
            .map_err(|err| err.to_string())
        })
        .unwrap_or(false)
    }

    fn view_path(&self, path: &Path) -> Result<(), String> {
        activity_result!("viewPath", &path.to_string_lossy())
    }

    fn view_apps(&self) -> Vec<(String, String)> {
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let apps = env
                .call_static_method(
                    &class,
                    jni::jni_str!("viewApps"),
                    jni::jni_sig!("()Ljava/lang/String;"),
                    &[],
                )
                .and_then(|value| value.l())
                .map_err(|err| err.to_string())?;
            Ok(gpui_android::jni::get_string(env, &apps))
        })
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(label, component)| (label.to_string(), component.to_string()))
        .collect()
    }

    fn view_path_with(&self, path: &Path, component: &str) -> Result<(), String> {
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let path = env
                .new_string(path.to_string_lossy())
                .map_err(|err| err.to_string())?;
            let component = env.new_string(component).map_err(|err| err.to_string())?;
            let message = env
                .call_static_method(
                    &class,
                    jni::jni_str!("viewPathWith"),
                    jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                    &[
                        jni::objects::JValue::Object(&path),
                        jni::objects::JValue::Object(&component),
                    ],
                )
                .and_then(|value| value.l())
                .map_err(|err| err.to_string())?;
            Ok(gpui_android::jni::get_string(env, &message))
        })
        .and_then(|message| {
            if message.is_empty() {
                Ok(())
            } else {
                Err(message)
            }
        })
    }

    fn view_path_with_chooser(&self, path: &Path) -> Result<(), String> {
        activity_result!("choosePath", &path.to_string_lossy())
    }

    fn share_path(&self, path: &Path) -> Result<(), String> {
        activity_result!("sharePath", &path.to_string_lossy())
    }

    fn open_termux(&self, dir: &Path) -> Result<(), String> {
        activity_result!("openTermux", &dir.to_string_lossy())
    }

    fn transfer_active(&self, active: bool) {
        activity_call!(
            "transferActive",
            "(Z)V",
            &[jni::objects::JValue::Bool(active)]
        );
    }
}

/// `Platform::open_with_system` and `Platform::reveal_path`.
fn open_path(path: &Path, reveal: bool) {
    let target = match path.parent() {
        Some(parent) if reveal && !path.is_dir() => parent,
        _ => path,
    };
    if let Err(err) = corvane_platform::android::view_path(target) {
        tracing::warn!(path = %path.display(), "could not open: {err}");
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeNetworkBusy(
    _env: *mut c_void,
    _class: *mut c_void,
) -> u8 {
    u8::from(corvane_platform::android::network_busy())
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeBackgroundFetch(
    _env: *mut c_void,
    _class: *mut c_void,
) -> u8 {
    u8::from(corvane_platform::android::background_fetch())
}

/// `Platform::open_url`: sign-in pages open in a Custom Tab over Corvane (the
/// browser flow comes back through the `x-corvane-auth` deep link, the
/// device flow's page is closed by hand), the notification settings URL
/// opens the system's page, everything else goes to the default handler.
fn open_url(url: &str) -> bool {
    if url == corvane_platform::android::NOTIFICATION_SETTINGS_URL {
        activity_call!("openNotificationSettings", "()V", &[]);
        return true;
    }
    let sign_in = url.starts_with("https://")
        && (url.contains("/login/oauth/authorize") || url.contains("/login/device"));
    if sign_in {
        return activity_strings!("openCustomTab", "(Ljava/lang/String;)V", [url]);
    }
    false
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeNotificationClicked(
    _env: *mut c_void,
    _class: *mut c_void,
    identifier: *mut c_void,
    payload: *mut c_void,
) {
    let payload = gpui_android::jni::string_from_raw(payload);
    corvane_platform::notifications::clicked(corvane_platform::notifications::NotificationClick {
        identifier: gpui_android::jni::string_from_raw(identifier),
        payload: (!payload.is_empty()).then_some(payload),
    });
}

/// `GrammarModule` (Google Play's on-demand module with the tree-sitter
/// grammars): 0 progress, 1 installed, 2 failed.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeGrammarModule(
    _env: *mut c_void,
    _class: *mut c_void,
    status: i32,
    received: i64,
    total: i64,
    error: *mut c_void,
) {
    use corvane_platform::android::GrammarModuleEvent;
    corvane_platform::android::grammar_module_event(match status {
        0 => GrammarModuleEvent::Progress {
            received: received.max(0) as u64,
            total: total.max(0) as u64,
        },
        1 => GrammarModuleEvent::Installed,
        _ => GrammarModuleEvent::Failed(gpui_android::jni::string_from_raw(error)),
    });
}

/// The folder prompt in flight: Android shows one picker at a time.
static PICKING: std::sync::Mutex<Option<gpui_android::PathPromptReply>> =
    std::sync::Mutex::new(None);

/// `Platform::prompt_for_paths`: the system's folder picker
/// (`CorvaneActivity.pickFolder`), which answers with a path Corvane can use:
/// a folder of its own storage, one on shared storage with "All files
/// access", or the imported copy of a repository picked anywhere else.
fn pick_folder(options: gpui_kit::PathPromptOptions, reply: gpui_android::PathPromptReply) {
    if !options.directories {
        // nothing in Corvane picks single files on Android yet
        reply(None);
        return;
    }
    let previous = PICKING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replace(reply);
    if let Some(previous) = previous {
        previous(None);
    }
    if activity_call!("pickFolder", "()V", &[]).is_none() {
        let reply = PICKING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(reply) = reply {
            reply(None);
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativePathPicked(
    _env: *mut c_void,
    _class: *mut c_void,
    path: *mut c_void,
    error: *mut c_void,
) {
    let path = gpui_android::jni::string_from_raw(path);
    let error = gpui_android::jni::string_from_raw(error);
    if !error.is_empty() {
        tracing::warn!("folder picker: {error}");
    }
    let path = (!path.is_empty()).then(|| PathBuf::from(path));
    if let Some(path) = &path {
        // a copy made through the Storage Access Framework has no file modes
        if path.starts_with(corvane_platform::android::repositories_dir())
            && path.join(".git").exists()
        {
            corvane_platform::android::note_imported(path);
        }
    }
    let reply = PICKING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(reply) = reply {
        reply(path);
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
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeInsets(
    _env: *mut c_void,
    _class: *mut c_void,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) {
    gpui_android::set_window_insets(left, top, right, bottom);
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

/// Answers the askpass helper (`crates/corvane-askpass`): git runs it for a
/// username or password, it connects to `CORVANE_ASKPASS_SOCKET` in the
/// app-private cache directory, and the answer comes from the Android
/// Keystore as on the desktop it comes from the keychain.
fn serve_askpass() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    let Some(socket) = std::env::var_os("CORVANE_ASKPASS_SOCKET") else {
        return;
    };
    let _ = std::fs::remove_file(&socket);
    let listener = match UnixListener::bind(&socket) {
        Ok(listener) => listener,
        Err(err) => {
            tracing::warn!("askpass socket: {err}");
            return;
        }
    };
    let spawned = std::thread::Builder::new()
        .name("askpass".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut lines = BufReader::new(&stream).lines();
                let (Some(Ok(logins)), Some(Ok(prompt))) = (lines.next(), lines.next()) else {
                    continue;
                };
                let answer =
                    app::askpass::answer_with(&prompt, app::askpass::parse_logins(&logins))
                        .unwrap_or_default();
                let _ = (&stream).write_all(answer.as_bytes());
            }
        });
    if let Err(err) = spawned {
        tracing::warn!("askpass thread: {err}");
    }
}

// ── the background fetch without an activity ────────────────────────────────
//
// WorkManager starts the process for `CorvaneFetchWorker` also when the
// application is not open. Then there is no `android_main`: the worker calls
// `nativeHeadlessFetch`, which sets up what `android_main` would (the
// environment, the Keystore's Java context, the askpass socket) and runs
// `corvane_core::headless::background_fetch`. Should the user open the
// application meanwhile, the activity calls `nativeEndHeadless` before it
// starts the native side: git is stopped, the fetch returns, and the Java
// context is handed back so android-activity can install its own.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Headless {
    Idle,
    Running,
    /// An activity was created: this process belongs to the application.
    Activity,
}

static HEADLESS: std::sync::Mutex<Headless> = std::sync::Mutex::new(Headless::Idle);
static HEADLESS_DONE: std::sync::Condvar = std::sync::Condvar::new();
static HEADLESS_CANCEL: std::sync::Mutex<Option<corvane_core::headless::CancelToken>> =
    std::sync::Mutex::new(None);

/// `CorvaneActivity.nativeHeadlessFetch(Context, String)`: true when a fetch
/// ran to its end.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeHeadlessFetch(
    env: *mut jni::sys::JNIEnv,
    _class: *mut c_void,
    context: jni::sys::jobject,
    files_dir: jni::sys::jstring,
) -> u8 {
    {
        let mut state = HEADLESS.lock().unwrap_or_else(|p| p.into_inner());
        if *state != Headless::Idle || ANDROID_APP.get().is_some() {
            return 0;
        }
        *state = Headless::Running;
    }
    let fetched =
        std::panic::catch_unwind(|| headless_fetch(env, context, files_dir)).unwrap_or(false);
    let mut state = HEADLESS.lock().unwrap_or_else(|p| p.into_inner());
    if *state == Headless::Running {
        *state = Headless::Idle;
    }
    HEADLESS_DONE.notify_all();
    u8::from(fetched)
}

fn headless_fetch(
    env: *mut jni::sys::JNIEnv,
    context: jni::sys::jobject,
    files_dir: jni::sys::jstring,
) -> bool {
    // SAFETY: `env` is the JNI environment of the running native call and
    // `context` / `files_dir` are its arguments
    let prepared = unsafe {
        let mut unowned = jni::EnvUnowned::from_raw(env);
        unowned
            .with_env(|env| -> jni::errors::Result<_> {
                let vm = env.get_java_vm()?;
                let context = jni::objects::JObject::from_raw(env, context);
                let global = env.new_global_ref(&context)?;
                let files = jni::objects::JString::from_raw(env, files_dir);
                Ok((vm, global, files.try_to_string(env)?))
            })
            .into_outcome()
    };
    let (vm, context, files) = match prepared {
        jni::Outcome::Ok(prepared) => prepared,
        _ => return false,
    };
    let files = PathBuf::from(files);
    // SAFETY: nothing else in the process reads the environment (no
    // `android_main` ran), and the context is taken back below before an
    // activity can start (`nativeEndHeadless` waits for this function)
    unsafe {
        prepare_environment(&files);
        ndk_context::initialize_android_context(vm.get_raw().cast(), context.as_raw().cast());
    }
    let cancel = corvane_core::headless::CancelToken::new();
    *HEADLESS_CANCEL.lock().unwrap_or_else(|p| p.into_inner()) = Some(cancel.clone());
    corvane_core::headless::set_cancel_token(Some(cancel));
    serve_askpass();

    // `paths::app_support_dir` follows HOME, which is set now
    let outcome =
        corvane_core::headless::background_fetch(&corvane_platform::paths::app_support_dir());
    match &outcome {
        Ok(outcome) => log_line(&format!("headless background fetch: {outcome:?}")),
        Err(err) => log_line(&format!("headless background fetch failed: {err}")),
    }

    corvane_core::headless::set_cancel_token(None);
    *HEADLESS_CANCEL.lock().unwrap_or_else(|p| p.into_inner()) = None;
    // SAFETY: initialised above; nothing uses the Keystore after the fetch
    unsafe { ndk_context::release_android_context() };
    drop(context);
    matches!(outcome, Ok(corvane_core::headless::Outcome::Fetched))
}

/// logcat without the tracing setup `main` installs.
fn log_line(message: &str) {
    if let Ok(message) = std::ffi::CString::new(message) {
        // SAFETY: two NUL-terminated strings
        unsafe {
            __android_log_write(4, c"corvane".as_ptr(), message.as_ptr());
        }
    }
}

unsafe extern "C" {
    fn __android_log_write(
        priority: std::ffi::c_int,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> std::ffi::c_int;
}

/// `CorvaneActivity.nativeEndHeadless()`, from `onCreate` before the native
/// side starts: stops a headless fetch and waits until it has let go.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvane_CorvaneActivity_nativeEndHeadless(
    _env: *mut c_void,
    _class: *mut c_void,
) {
    if let Some(cancel) = HEADLESS_CANCEL
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        cancel.cancel();
    }
    let mut state = HEADLESS.lock().unwrap_or_else(|p| p.into_inner());
    while *state == Headless::Running {
        state = HEADLESS_DONE.wait(state).unwrap_or_else(|p| p.into_inner());
    }
    *state = Headless::Activity;
}
