package com.wasimaster.corvene.ffi

import android.content.Context

/**
 * The engine's one JNI entry point (crates/corvene-ffi/src/jni.rs): hands
 * the JVM and the application context to `ndk_context`, which the
 * Keystore-backed token store reaches Android through. JNA loads the library
 * without that, so [Native.prepare] calls this before `Corvene(...)` or
 * `headlessFetch(...)`; without it signing in cannot save a token.
 */
object NativeContext {
    external fun attach(context: Context)
}

/** Loads the engine and attaches the context; safe to call more than once. */
object Native {
    @Volatile
    private var prepared = false

    @Synchronized
    fun prepare(context: Context) {
        if (prepared) return
        NativeLoader.load()
        NativeContext.attach(context.applicationContext)
        prepared = true
    }
}
