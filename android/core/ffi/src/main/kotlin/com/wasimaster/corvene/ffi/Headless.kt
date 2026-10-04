package com.wasimaster.corvene.ffi

import android.content.Context
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.gen.HeadlessOutcome
import com.wasimaster.corvene.ffi.gen.endHeadless
import com.wasimaster.corvene.ffi.gen.headlessFetch

/**
 * The background fetch in a process without the app (WorkManager started
 * it for the fetch worker): the engine fetches the selected repository by
 * itself, with the credentials in the Keystore. [end] stops it when the
 * activity starts in that process (MainActivity, before [Core.start]).
 */
object Headless {
    /** Blocking: run it on the worker's thread. */
    fun fetch(context: Context): HeadlessOutcome {
        Native.prepare(context)
        return headlessFetch(context.filesDir.path)
    }

    /** Stops a running [fetch]; nothing to do when none runs (or the library is not loaded). */
    fun end() {
        if (!NativeLoader.isLoaded) return
        try {
            endHeadless()
        } catch (error: UnsatisfiedLinkError) {
            CorveneLog.w("endHeadless", error)
        }
    }
}
