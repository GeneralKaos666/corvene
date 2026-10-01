//! JNI helpers over the activity `android-activity` hands to `android_main`.
//!
//! Ported from gpui-mobile's `src/android/jni.rs`, which also held the event
//! loop (now `event_loop.rs`) and process-global platform state (now owned
//! by `AndroidPlatform` on the main thread).

use std::sync::OnceLock;

use android_activity::AndroidApp;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::JavaVM;

static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

/// Remembers the application; called once when the platform is created.
pub(crate) fn init(app: &AndroidApp) {
    let _ = ANDROID_APP.set(app.clone());
}

/// The `AndroidApp` of this process, once the platform exists.
pub fn android_app() -> Option<AndroidApp> {
    ANDROID_APP.get().cloned()
}

fn java_vm() -> Result<&'static JavaVM, String> {
    if let Some(vm) = JAVA_VM.get() {
        return Ok(vm);
    }
    let app = ANDROID_APP.get().ok_or("the platform is not initialised")?;
    let ptr = app.vm_as_ptr();
    if ptr.is_null() {
        return Err("no JavaVM".into());
    }
    // SAFETY: android-activity hands out the process's `JavaVM *`
    Ok(JAVA_VM.get_or_init(|| unsafe { JavaVM::from_raw(ptr.cast()) }))
}

/// Runs `f` with a JNI environment for the current thread, attaching it when
/// needed, inside a local reference frame. A pending Java exception is
/// cleared and reported as the error.
pub fn with_env<T>(f: impl FnOnce(&mut jni::Env) -> Result<T, String>) -> Result<T, String> {
    let vm = java_vm()?;
    let mut result = None;
    let attached = vm.attach_current_thread(|env: &mut jni::Env| -> jni::errors::Result<()> {
        let value = f(env);
        if env.exception_check() {
            env.exception_describe();
            env.exception_clear();
        }
        result = Some(value);
        Ok(())
    });
    match (attached, result) {
        (Ok(()), Some(result)) => result,
        (Err(err), _) => Err(err.to_string()),
        (Ok(()), None) => Err("the JNI callback did not run".into()),
    }
}

/// The activity as a `JObject`: a global reference owned by
/// `android-activity`, valid for the life of the activity.
pub fn activity<'local>(env: &jni::Env<'local>) -> Result<JObject<'local>, String> {
    let app = ANDROID_APP.get().ok_or("the platform is not initialised")?;
    let ptr = app.activity_as_ptr();
    if ptr.is_null() {
        return Err("no activity".into());
    }
    // SAFETY: see above; the wrapper never deletes the reference
    Ok(unsafe { JObject::from_raw(env, ptr.cast()) })
}

/// A Java string as a Rust `String`; empty for `null` or on error.
pub fn get_string(env: &mut jni::Env<'_>, obj: &JObject<'_>) -> String {
    if obj.is_null() {
        return String::new();
    }
    // SAFETY: callers pass `java.lang.String` objects
    let string = unsafe { JString::from_raw(env, obj.as_raw()) };
    string.try_to_string(env).unwrap_or_default()
}

/// Maps a JNI error onto the `String` errors `with_env` closures return.
pub trait JniExt<T> {
    fn e(self) -> Result<T, String>;
}

impl<T> JniExt<T> for jni::errors::Result<T> {
    fn e(self) -> Result<T, String> {
        self.map_err(|err| err.to_string())
    }
}

/// Loads a class of the application (dot notation) through the activity's
/// class loader: `FindClass` on a native thread only sees framework classes.
pub fn find_app_class<'local>(
    env: &mut jni::Env<'local>,
    class_name: &str,
) -> Result<JClass<'local>, String> {
    let activity = activity(env)?;
    let class_loader = env
        .call_method(
            &activity,
            jni::jni_str!("getClassLoader"),
            jni::jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .and_then(|value| value.l())
        .e()?;
    let name = env.new_string(class_name).e()?;
    let class = env
        .call_method(
            &class_loader,
            jni::jni_str!("loadClass"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )
        .and_then(|value| value.l())
        .e()?;
    // SAFETY: `loadClass` returns a `java.lang.Class`
    Ok(unsafe { JClass::from_raw(env, class.as_raw()) })
}

fn system_service<'local>(
    env: &mut jni::Env<'local>,
    name: &str,
) -> Result<JObject<'local>, String> {
    let activity = activity(env)?;
    let name = env.new_string(name).e()?;
    let service = env
        .call_method(
            &activity,
            jni::jni_str!("getSystemService"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
            &[JValue::Object(&name)],
        )
        .and_then(|value| value.l())
        .e()?;
    if service.is_null() {
        return Err("no such system service".into());
    }
    Ok(service)
}

/// The text on the clipboard (`ClipboardManager.getPrimaryClip`), which
/// Android only shows to the focused application.
pub(crate) fn clipboard_text() -> Option<String> {
    with_env(|env| {
        let manager = system_service(env, "clipboard")?;
        let clip = env
            .call_method(
                &manager,
                jni::jni_str!("getPrimaryClip"),
                jni::jni_sig!("()Landroid/content/ClipData;"),
                &[],
            )
            .and_then(|value| value.l())
            .e()?;
        if clip.is_null() {
            return Ok(None);
        }
        let count = env
            .call_method(
                &clip,
                jni::jni_str!("getItemCount"),
                jni::jni_sig!("()I"),
                &[],
            )
            .and_then(|value| value.i())
            .e()?;
        if count < 1 {
            return Ok(None);
        }
        let item = env
            .call_method(
                &clip,
                jni::jni_str!("getItemAt"),
                jni::jni_sig!("(I)Landroid/content/ClipData$Item;"),
                &[JValue::Int(0)],
            )
            .and_then(|value| value.l())
            .e()?;
        let activity = activity(env)?;
        let text = env
            .call_method(
                &item,
                jni::jni_str!("coerceToText"),
                jni::jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                &[JValue::Object(&activity)],
            )
            .and_then(|value| value.l())
            .e()?;
        if text.is_null() {
            return Ok(None);
        }
        let string = env
            .call_method(
                &text,
                jni::jni_str!("toString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .and_then(|value| value.l())
            .e()?;
        Ok(Some(get_string(env, &string)))
    })
    .unwrap_or_else(|err| {
        log::warn!("reading the clipboard failed: {err}");
        None
    })
}

/// Puts `text` on the clipboard (`ClipboardManager.setPrimaryClip`).
pub(crate) fn set_clipboard_text(text: &str) {
    let result = with_env(|env| {
        let manager = system_service(env, "clipboard")?;
        let label = env.new_string("").e()?;
        let text = env.new_string(text).e()?;
        let clip = env
            .call_static_method(
                jni::jni_str!("android/content/ClipData"),
                jni::jni_str!("newPlainText"),
                jni::jni_sig!(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                ),
                &[JValue::Object(&label), JValue::Object(&text)],
            )
            .and_then(|value| value.l())
            .e()?;
        env.call_method(
            &manager,
            jni::jni_str!("setPrimaryClip"),
            jni::jni_sig!("(Landroid/content/ClipData;)V"),
            &[JValue::Object(&clip)],
        )
        .e()?;
        Ok(())
    });
    if let Err(err) = result {
        log::warn!("writing the clipboard failed: {err}");
    }
}

/// `startActivity(new Intent(ACTION_VIEW, Uri.parse(url)))`.
pub(crate) fn open_url(url: &str) {
    const FLAG_ACTIVITY_NEW_TASK: i32 = 0x1000_0000;
    let result = with_env(|env| {
        let url = env.new_string(url).e()?;
        let uri = env
            .call_static_method(
                jni::jni_str!("android/net/Uri"),
                jni::jni_str!("parse"),
                jni::jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&url)],
            )
            .and_then(|value| value.l())
            .e()?;
        let action = env.new_string("android.intent.action.VIEW").e()?;
        let intent = env
            .new_object(
                jni::jni_str!("android/content/Intent"),
                jni::jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
                &[JValue::Object(&action), JValue::Object(&uri)],
            )
            .e()?;
        env.call_method(
            &intent,
            jni::jni_str!("addFlags"),
            jni::jni_sig!("(I)Landroid/content/Intent;"),
            &[JValue::Int(FLAG_ACTIVITY_NEW_TASK)],
        )
        .e()?;
        let activity = activity(env)?;
        env.call_method(
            &activity,
            jni::jni_str!("startActivity"),
            jni::jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        )
        .e()?;
        Ok(())
    });
    if let Err(err) = result {
        log::warn!("opening a URL failed: {err}");
    }
}

/// The intent that started the activity carried this URL
/// (`getIntent().getDataString()`), e.g. a deep link that launched the app.
pub(crate) fn launch_url() -> Option<String> {
    with_env(|env| {
        let activity = activity(env)?;
        let intent = env
            .call_method(
                &activity,
                jni::jni_str!("getIntent"),
                jni::jni_sig!("()Landroid/content/Intent;"),
                &[],
            )
            .and_then(|value| value.l())
            .e()?;
        if intent.is_null() {
            return Ok(None);
        }
        let data = env
            .call_method(
                &intent,
                jni::jni_str!("getDataString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .and_then(|value| value.l())
            .e()?;
        let url = get_string(env, &data);
        Ok((!url.is_empty()).then_some(url))
    })
    .ok()
    .flatten()
}
