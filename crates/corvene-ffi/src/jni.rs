//! The one JNI entry point: the Android `Context` for the Keystore.
//!
//! `android-native-keyring-store` (the token store behind
//! `corvene_platform::keychain`) reaches the JVM through `ndk_context`,
//! which `NativeActivity` initialises in the GPUI app. JNA loads this
//! library without any of that, so Kotlin calls
//! `com.wasimaster.corvene.ffi.NativeContext.attach(applicationContext)`
//! once, before `Corvene` is created or a headless fetch starts.

#![cfg(target_os = "android")]

use std::sync::atomic::{AtomicBool, Ordering};

static ATTACHED: AtomicBool = AtomicBool::new(false);

/// `external fun attach(context: Context)` on `NativeContext`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_wasimaster_corvene_ffi_NativeContext_attach(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    context: jni::sys::jobject,
) {
    if ATTACHED.swap(true, Ordering::SeqCst) {
        return;
    }
    // SAFETY: `env` is the JNI environment of the running native call and
    // `context` is its argument
    let prepared = unsafe {
        let mut unowned = jni::EnvUnowned::from_raw(env);
        unowned
            .with_env(|env| -> jni::errors::Result<_> {
                let vm = env.get_java_vm()?;
                let context = jni::objects::JObject::from_raw(env, context);
                let global = env.new_global_ref(&context)?;
                Ok((vm, global))
            })
            .into_outcome()
    };
    let jni::Outcome::Ok((vm, context)) = prepared else {
        ATTACHED.store(false, Ordering::SeqCst);
        return;
    };
    // SAFETY: the global reference lives for the process (leaked below),
    // and this runs once
    unsafe {
        ndk_context::initialize_android_context(vm.get_raw().cast(), context.as_raw().cast());
    }
    std::mem::forget(context);
}

/// `true` once [`Java_com_wasimaster_corvene_ffi_NativeContext_attach`] ran.
pub fn attached() -> bool {
    ATTACHED.load(Ordering::SeqCst)
}
