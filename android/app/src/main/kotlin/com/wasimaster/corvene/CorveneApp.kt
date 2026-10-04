package com.wasimaster.corvene

import android.app.Application
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ProcessLifecycleOwner
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.CoreConfig
import com.wasimaster.corvene.ffi.gen.HostInfo
import com.wasimaster.corvene.platform.CorveneFetchWorker
import com.wasimaster.corvene.platform.FolderResolver
import com.wasimaster.corvene.platform.Notifications

/**
 * The process: the notification channels, the hourly background fetch
 * ([CorveneFetchWorker]), and the engine once an activity asks for [core]
 * (MainActivity, first thing): a process WorkManager started for the fetch
 * has no activity and runs the engine headless instead. Tells the engine
 * when the app is on screen (`appVisible`, ProcessLifecycleOwner) and comes
 * to the front (`focus`). Explicit wiring, no DI: [core] is the one engine,
 * handed to the UI through `LocalCore`.
 */
class CorveneApp : Application() {

    val core: Core by lazy {
        Core.start(this) {
            CoreConfig(
                filesDir = filesDir.path,
                env = mapOf("CORVENE_LOG" to "info"),
                info = HostInfo(
                    hasAllFilesAccess = FolderResolver.hasAllFilesAccess(this),
                    canRequestAllFilesAccess = FolderResolver.canRequestAllFilesAccess(this),
                    allowsDownloadedCode = BuildConfig.DOWNLOADED_CODE,
                    grammarModuleDir = null,
                    notificationsAllowed = Notifications.allowed(this),
                ),
            )
        }
    }

    override fun onCreate() {
        super.onCreate()
        CorveneTrace.section(CorveneTrace.STARTUP) {
            CorveneLog.i("Corvene ${BuildConfig.VERSION_NAME} (${BuildConfig.FLAVOR}) starting")
            Notifications.createChannels(this)
            CorveneFetchWorker.schedule(this)
        }
        // GHD pauses its periodic work while the window is hidden and refreshes on focus
        ProcessLifecycleOwner.get().lifecycle.addObserver(
            LifecycleEventObserver { _, event ->
                when (event) {
                    // only an activity starts the process lifecycle, and MainActivity has made the engine by then
                    Lifecycle.Event.ON_START -> core.dispatch { appVisible(true) }
                    Lifecycle.Event.ON_STOP -> core.dispatch { appVisible(false) }
                    Lifecycle.Event.ON_RESUME -> core.dispatch { focus() }
                    Lifecycle.Event.ON_CREATE, Lifecycle.Event.ON_PAUSE, Lifecycle.Event.ON_DESTROY, Lifecycle.Event.ON_ANY -> Unit
                }
            },
        )
    }
}
