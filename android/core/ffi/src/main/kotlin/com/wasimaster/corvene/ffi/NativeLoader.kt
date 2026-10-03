package com.wasimaster.corvene.ffi

import com.wasimaster.corvene.common.CorveneLog

/**
 * Loads `libcorvene_ffi.so` before the bindings' first call, so a missing or
 * broken library fails here with a message instead of inside JNA's proxy.
 * JNA then finds the already-loaded library by name.
 */
internal object NativeLoader {
    const val LIBRARY = "corvene_ffi"

    @Volatile
    private var loaded = false

    @Synchronized
    fun load() {
        if (loaded) return
        val started = System.nanoTime()
        try {
            System.loadLibrary(LIBRARY)
        } catch (error: UnsatisfiedLinkError) {
            throw IllegalStateException("lib$LIBRARY.so is not in this package (built for another ABI?)", error)
        }
        loaded = true
        CorveneLog.i("loaded lib$LIBRARY.so in ${(System.nanoTime() - started) / NANOS_PER_MILLI} ms")
    }

    private const val NANOS_PER_MILLI = 1_000_000
}
