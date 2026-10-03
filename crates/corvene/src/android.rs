//! Android entry point. The activity (`packaging/android`) is a
//! `NativeActivity`: it loads this library, `libcorvene.so`, and
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
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/corvene"));
    corvene_platform::android::prepare_environment(&files);
    let _ = ANDROID_APP.set(android_app);
    gpui_android::set_soft_keyboard_handler(show_keyboard);
    gpui_android::set_path_prompt_handler(pick_folder);
    gpui_android::set_url_handler(open_url);
    gpui_android::set_path_opener(open_path);
    gpui_android::set_touch_as_mouse(corvene_platform::android::drag_handle_at);
    // The render scale the platform settled on last time (it lowers the
    // resolution it draws at when the GPU cannot keep up), so a start does
    // not begin slow again.
    let render_scale = files.join("render-scale");
    gpui_android::set_render_scale(
        std::fs::read_to_string(&render_scale)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(1.0),
        move |scale| {
            let _ = std::fs::write(&render_scale, scale.to_string());
        },
    );
    corvene_platform::android::set_bridge(Box::new(ActivityBridge));
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

// ── CorveneActivity (packaging/android) ─────────────────────────────────────
//
// The activity's input view forwards what the keyboard does; these run on
// the Java UI thread and only queue the event for the native thread.

use std::ffi::c_void;

use gpui_android::ActivityEvent;

const ACTIVITY: &str = "com.wasimaster.corvene.CorveneActivity";

/// Calls a static method of `CorveneActivity` that returns `boolean` or
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
                tracing::warn!("CorveneActivity.{} failed: {err}", $name);
                None
            }
        }
    }};
}

/// A static `int` method of `CorveneActivity` without arguments.
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

/// A static `void` method of `CorveneActivity` taking strings; true when the
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
            tracing::warn!("CorveneActivity.{} failed: {err}", $name);
        }
        result.is_ok()
    }};
}

/// A static method of `CorveneActivity` that takes one string and returns an
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

/// `CorveneActivity.showKeyboard(boolean)`.
fn show_keyboard(show: bool) {
    activity_call!("showKeyboard", "(Z)V", &[jni::objects::JValue::Bool(show)]);
}

struct ActivityBridge;

impl corvene_platform::android::Bridge for ActivityBridge {
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

    fn app_icon(&self, key: &str) -> Option<Vec<u8>> {
        let path = gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let key = env.new_string(key).map_err(|err| err.to_string())?;
            let path = env
                .call_static_method(
                    &class,
                    jni::jni_str!("appIcon"),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[jni::objects::JValue::Object(&key)],
                )
                .and_then(|value| value.l())
                .map_err(|err| err.to_string())?;
            Ok(gpui_android::jni::get_string(env, &path))
        })
        .ok()
        .filter(|path| !path.is_empty())?;
        std::fs::read(path).ok()
    }

    fn view_path_with(
        &self,
        path: &Path,
        component: &str,
        line: Option<u32>,
    ) -> Result<(), String> {
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let path = env
                .new_string(path.to_string_lossy())
                .map_err(|err| err.to_string())?;
            let component = env.new_string(component).map_err(|err| err.to_string())?;
            let message = env
                .call_static_method(
                    &class,
                    jni::jni_str!("viewPathAt"),
                    jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)Ljava/lang/String;"),
                    &[
                        jni::objects::JValue::Object(&path),
                        jni::objects::JValue::Object(&component),
                        jni::objects::JValue::Int(line.unwrap_or(0) as i32),
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

    fn run_termux(&self, program: &str, arguments: &[String], dir: &Path) -> Result<(), String> {
        let arguments = arguments.join("\n");
        let dir = dir.to_string_lossy();
        gpui_android::jni::with_env(|env| {
            let class = gpui_android::jni::find_app_class(env, ACTIVITY)?;
            let strings = [
                env.new_string(program).map_err(|err| err.to_string())?,
                env.new_string(&arguments).map_err(|err| err.to_string())?,
                env.new_string(&*dir).map_err(|err| err.to_string())?,
            ];
            let args: Vec<jni::objects::JValue> = strings
                .iter()
                .map(|s| jni::objects::JValue::Object(s))
                .collect();
            let message = env
                .call_static_method(
                    &class,
                    jni::jni_str!("runTermux"),
                    jni::jni_sig!(
                        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"
                    ),
                    &args,
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

    fn termux_programs(&self, candidates: &[&str]) -> Option<Vec<String>> {
        let found: Result<(), String> = activity_result!("termuxEditors", &candidates.join(" "));
        // the method's "error message" is its answer
        let found = match found {
            Ok(()) => String::new(),
            Err(found) => found,
        };
        (found != "?").then(|| {
            found
                .lines()
                .filter(|line| candidates.contains(line))
                .map(str::to_string)
                .collect()
        })
    }

    fn transfer_active(&self, active: bool) {
        activity_call!(
            "transferActive",
            "(Z)V",
            &[jni::objects::JValue::Bool(active)]
        );
    }

    fn toast(&self, message: &str) {
        let _ = activity_result!("toast", message);
    }

    fn bring_to_front(&self) {
        activity_call!("bringToFront", "()V", &[]);
    }

    fn relaunch(&self) {
        activity_call!("relaunch", "()V", &[]);
    }

    fn toggle_full_screen(&self) {
        activity_call!("toggleFullScreen", "()V", &[]);
    }
}

/// `Platform::open_with_system` and `Platform::reveal_path`.
fn open_path(path: &Path, reveal: bool) {
    let target = match path.parent() {
        Some(parent) if reveal && !path.is_dir() => parent,
        _ => path,
    };
    if let Err(err) = corvene_platform::android::view_path(target) {
        tracing::warn!(path = %path.display(), "could not open: {err}");
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeNetworkBusy(
    _env: *mut c_void,
    _class: *mut c_void,
) -> u8 {
    u8::from(corvene_platform::android::network_busy())
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeBackgroundFetch(
    _env: *mut c_void,
    _class: *mut c_void,
) -> u8 {
    u8::from(corvene_platform::android::background_fetch())
}

/// `Platform::open_url`: sign-in pages open in a Custom Tab over Corvene (the
/// browser flow comes back through the `x-corvene-auth` deep link, the
/// device flow's page is closed by hand), the notification settings URL
/// opens the system's page, everything else goes to the default handler.
fn open_url(url: &str) -> bool {
    if url == corvene_platform::android::NOTIFICATION_SETTINGS_URL {
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeNotificationClicked(
    _env: *mut c_void,
    _class: *mut c_void,
    identifier: *mut c_void,
    payload: *mut c_void,
) {
    let payload = gpui_android::jni::string_from_raw(payload);
    corvene_platform::notifications::clicked(corvene_platform::notifications::NotificationClick {
        identifier: gpui_android::jni::string_from_raw(identifier),
        payload: (!payload.is_empty()).then_some(payload),
    });
}

/// `GrammarModule` (Google Play's on-demand module with the tree-sitter
/// grammars): 0 progress, 1 installed, 2 failed.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeGrammarModule(
    _env: *mut c_void,
    _class: *mut c_void,
    status: i32,
    received: i64,
    total: i64,
    error: *mut c_void,
) {
    use corvene_platform::android::GrammarModuleEvent;
    corvene_platform::android::grammar_module_event(match status {
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
/// (`CorveneActivity.pickFolder`), which answers with a path Corvene can use:
/// a folder of its own storage, one on shared storage with "All files
/// access", or the imported copy of a repository picked anywhere else.
fn pick_folder(options: gpui_kit::PathPromptOptions, reply: gpui_android::PathPromptReply) {
    let previous = PICKING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replace(reply);
    if let Some(previous) = previous {
        previous(None);
    }
    // a file is copied into the cache directory, whose path is the answer
    let shown = if options.directories {
        activity_call!("pickFolder", "()V", &[])
    } else {
        activity_call!("pickFile", "()V", &[])
    };
    if shown.is_none() {
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativePathPicked(
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
        if path.starts_with(corvene_platform::android::repositories_dir())
            && path.join(".git").exists()
        {
            corvene_platform::android::note_imported(path);
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeCommitText(
    _env: *mut c_void,
    _class: *mut c_void,
    text: *mut c_void,
) {
    gpui_android::post(ActivityEvent::CommitText(
        gpui_android::jni::string_from_raw(text),
    ));
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeSetComposingText(
    _env: *mut c_void,
    _class: *mut c_void,
    text: *mut c_void,
) {
    gpui_android::post(ActivityEvent::SetComposingText(
        gpui_android::jni::string_from_raw(text),
    ));
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeFinishComposing(
    _env: *mut c_void,
    _class: *mut c_void,
) {
    gpui_android::post(ActivityEvent::FinishComposing);
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeDeleteSurrounding(
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeKey(
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeInsets(
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
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeOpenUrl(
    _env: *mut c_void,
    _class: *mut c_void,
    url: *mut c_void,
) {
    gpui_android::post(ActivityEvent::OpenUrl(gpui_android::jni::string_from_raw(
        url,
    )));
}

fn serve_askpass() {
    corvene_core::askpass::serve_socket();
}

// ── the background fetch without an activity ────────────────────────────────
//
// WorkManager starts the process for `CorveneFetchWorker` also when the
// application is not open. Then there is no `android_main`: the worker calls
// `nativeHeadlessFetch`, which sets up what `android_main` would (the
// environment, the Keystore's Java context, the askpass socket) and runs
// `corvene_core::headless::background_fetch`. Should the user open the
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
static HEADLESS_CANCEL: std::sync::Mutex<Option<corvene_core::headless::CancelToken>> =
    std::sync::Mutex::new(None);

/// `CorveneActivity.nativeHeadlessFetch(Context, String)`: true when a fetch
/// ran to its end.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeHeadlessFetch(
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
        corvene_platform::android::prepare_environment(&files);
        ndk_context::initialize_android_context(vm.get_raw().cast(), context.as_raw().cast());
    }
    let cancel = corvene_core::headless::CancelToken::new();
    *HEADLESS_CANCEL.lock().unwrap_or_else(|p| p.into_inner()) = Some(cancel.clone());
    corvene_core::headless::set_cancel_token(Some(cancel));
    serve_askpass();

    // `paths::app_support_dir` follows HOME, which is set now
    let outcome =
        corvene_core::headless::background_fetch(&corvene_platform::paths::app_support_dir());
    match &outcome {
        Ok(outcome) => log_line(&format!("headless background fetch: {outcome:?}")),
        Err(err) => log_line(&format!("headless background fetch failed: {err}")),
    }

    corvene_core::headless::set_cancel_token(None);
    *HEADLESS_CANCEL.lock().unwrap_or_else(|p| p.into_inner()) = None;
    // SAFETY: initialised above; nothing uses the Keystore after the fetch
    unsafe { ndk_context::release_android_context() };
    drop(context);
    matches!(outcome, Ok(corvene_core::headless::Outcome::Fetched))
}

/// logcat without the tracing setup `main` installs.
fn log_line(message: &str) {
    if let Ok(message) = std::ffi::CString::new(message) {
        // SAFETY: two NUL-terminated strings
        unsafe {
            __android_log_write(4, c"corvene".as_ptr(), message.as_ptr());
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

/// `CorveneActivity.nativeEndHeadless()`, from `onCreate` before the native
/// side starts: stops a headless fetch and waits until it has let go.
#[unsafe(no_mangle)]
extern "system" fn Java_com_wasimaster_corvene_CorveneActivity_nativeEndHeadless(
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
