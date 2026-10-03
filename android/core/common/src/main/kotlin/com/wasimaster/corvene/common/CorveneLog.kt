package com.wasimaster.corvene.common

import android.util.Log

/**
 * The app's log, under the tag the engine logs with too (`android_logger`
 * in crates/corvene-ffi), so `adb logcat -s corvene` shows both sides.
 */
object CorveneLog {
    const val TAG = "corvene"

    fun d(message: String) {
        Log.d(TAG, message)
    }

    fun i(message: String) {
        Log.i(TAG, message)
    }

    fun w(message: String, error: Throwable? = null) {
        Log.w(TAG, message, error)
    }

    fun e(message: String, error: Throwable? = null) {
        Log.e(TAG, message, error)
    }
}
