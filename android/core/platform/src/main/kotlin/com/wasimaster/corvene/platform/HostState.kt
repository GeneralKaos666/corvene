package com.wasimaster.corvene.platform

import android.Manifest
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.gen.HostInfo

/**
 * What the engine knows about the device ([HostInfo]): all-files access,
 * notifications, whether the build may run downloaded code. Read at start
 * (CorveneApp) and again whenever the app comes back to the front, so a
 * permission granted in Android's settings counts at once (`setHostInfo`).
 */
object HostState {
    /** The build's `DOWNLOADED_CODE` switch (foss true, play false); CorveneApp sets it before the engine starts. */
    @Volatile
    var allowsDownloadedCode: Boolean = true

    fun info(context: Context): HostInfo = HostInfo(
        hasAllFilesAccess = FolderResolver.hasAllFilesAccess(context),
        canRequestAllFilesAccess = FolderResolver.canRequestAllFilesAccess(context),
        allowsDownloadedCode = allowsDownloadedCode,
        grammarModuleDir = null,
        notificationsAllowed = Notifications.allowed(context),
    )

    /** Tells the engine what changed (a no-op for it when nothing did). */
    fun refresh(core: Core, context: Context) {
        val info = info(context.applicationContext)
        core.dispatch { setHostInfo(info) }
    }
}

/** Android's notification settings of this app (channels, the app switch). */
fun openNotificationSettings(context: Context) {
    val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
    if (context !is Activity) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        CorveneLog.w("no notification settings page")
    }
}

/**
 * Asks for POST_NOTIFICATIONS (Android 13+) and reports whether notifications
 * are allowed afterwards; below 13, or once Android stops asking, it opens
 * the app's notification settings instead.
 */
@Composable
fun rememberNotificationPermission(onResult: (allowed: Boolean) -> Unit): () -> Unit {
    val context = LocalContext.current
    val result = rememberUpdatedState(onResult)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        CorveneLog.i("notification permission: $granted")
        if (!granted && !Notifications.allowed(context)) openNotificationSettings(context)
        result.value(Notifications.allowed(context))
    }
    return {
        when {
            Notifications.allowed(context) -> result.value(true)
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU -> launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            else -> openNotificationSettings(context)
        }
    }
}
