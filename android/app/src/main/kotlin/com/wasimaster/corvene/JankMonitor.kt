package com.wasimaster.corvene

import android.app.Activity
import android.util.Log
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalView
import androidx.metrics.performance.JankStats
import androidx.metrics.performance.PerformanceMetricsState

/**
 * Janky frames in logcat, for judging scrolling by hand on a device without
 * Perfetto: JankStats over the activity's window, each frame tagged with the
 * Navigation 3 key on top ([JankScreenTag]). Off unless the tag is enabled,
 * so release builds pay nothing:
 *
 *     adb shell setprop log.tag.CorveneJank DEBUG
 *     adb logcat -s CorveneJank
 *
 * (The process reads the property when the activity starts; restart it after
 * setting it.)
 */
object JankMonitor {
    const val TAG = "CorveneJank"
    private const val NANOS_PER_MILLI = 1_000_000

    /** Whether `log.tag.CorveneJank` is DEBUG or lower. */
    val enabled: Boolean get() = Log.isLoggable(TAG, Log.DEBUG)

    /** Starts reporting the activity's janky frames (when [enabled]); the activity keeps the result and pauses it while stopped. */
    fun install(activity: Activity): JankStats? {
        if (!enabled) return null
        Log.d(TAG, "tracking janky frames")
        return JankStats.createAndTrack(activity.window) { frame ->
            if (frame.isJank) {
                val states = frame.states.joinToString(" ") { "${it.key}=${it.value}" }
                Log.d(TAG, "jank ${frame.frameDurationUiNanos / NANOS_PER_MILLI} ms $states")
            }
        }
    }
}

/** Tags the frames that follow with [key] (the screen on top), for [JankMonitor]'s lines. */
@Composable
fun JankScreenTag(key: Any?) {
    val view = LocalView.current
    LaunchedEffect(key) {
        if (!JankMonitor.enabled) return@LaunchedEffect
        PerformanceMetricsState.getHolderForHierarchy(view).state?.putState("screen", key?.toString() ?: "none")
    }
}
