package com.wasimaster.corvene

import android.app.Application
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ProcessLifecycleOwner
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.CoreConfig
import com.wasimaster.corvene.ffi.gen.HostInfo
import com.wasimaster.corvene.platform.FolderResolver

/**
 * The process: starts the engine on its own thread as early as possible, so
 * the store is loading while the activity inflates, and tells it when the app
 * is on screen (`appVisible`, ProcessLifecycleOwner) and comes to the front
 * (`focus`). Explicit wiring, no DI:
 * [core] is the one engine, handed to the UI through `LocalCore`.
 */
class CorveneApp : Application() {

    val core: Core by lazy {
        Core.start {
            CoreConfig(
                filesDir = filesDir.path,
                env = mapOf("CORVENE_LOG" to "info"),
                info = HostInfo(
                    hasAllFilesAccess = FolderResolver.hasAllFilesAccess(this),
                    canRequestAllFilesAccess = BuildConfig.FLAVOR == "foss" && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R,
                    allowsDownloadedCode = BuildConfig.DOWNLOADED_CODE,
                    grammarModuleDir = null,
                    notificationsAllowed = NotificationManagerCompat.from(this).areNotificationsEnabled(),
                ),
            )
        }
    }

    override fun onCreate() {
        super.onCreate()
        CorveneTrace.section(CorveneTrace.STARTUP) {
            CorveneLog.i("Corvene ${BuildConfig.VERSION_NAME} (${BuildConfig.FLAVOR}) starting")
            core
        }
        // GHD pauses its periodic work while the window is hidden and refreshes on focus
        ProcessLifecycleOwner.get().lifecycle.addObserver(
            LifecycleEventObserver { _, event ->
                when (event) {
                    Lifecycle.Event.ON_START -> core.dispatch { appVisible(true) }
                    Lifecycle.Event.ON_STOP -> core.dispatch { appVisible(false) }
                    Lifecycle.Event.ON_RESUME -> core.dispatch { focus() }
                    Lifecycle.Event.ON_CREATE, Lifecycle.Event.ON_PAUSE, Lifecycle.Event.ON_DESTROY, Lifecycle.Event.ON_ANY -> Unit
                }
            },
        )
    }
}
