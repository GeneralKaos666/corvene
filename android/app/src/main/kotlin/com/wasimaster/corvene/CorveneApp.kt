package com.wasimaster.corvene

import android.app.Application
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.CoreConfig
import com.wasimaster.corvene.ffi.gen.HostInfo
import com.wasimaster.corvene.platform.FolderResolver

/**
 * The process: starts the engine on its own thread as early as possible, so
 * the store is loading while the activity inflates. Explicit wiring, no DI:
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
    }
}
