package com.wasimaster.corvene.platform

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.net.toUri
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.HostRequest

/**
 * Handles the engine's [HostRequest]s while the activity is composed: URLs,
 * the clipboard, toasts and the folder picker (answered with `pathsPicked`).
 * Requests made while no activity is up wait in [Core.hostRequests].
 * Notifications, the transfer service, opening files and all-files access
 * arrive with M-A3 and are logged until then.
 */
@Composable
fun HostRequestHandler(core: Core, onQuit: () -> Unit) {
    val context = LocalContext.current
    // the engine's request id survives the activity being recreated under the picker
    var pendingPick by rememberSaveable { mutableStateOf<Long?>(null) }
    val picker = rememberFolderPicker { path ->
        val request = pendingPick ?: return@rememberFolderPicker
        pendingPick = null
        core.dispatch { pathsPicked(request.toULong(), path?.let(::listOf)) }
    }
    LaunchedEffect(core) {
        core.hostRequests.collect { request ->
            CorveneTrace.section(CorveneTrace.HOST_REQUEST) {
                when (request) {
                    is HostRequest.OpenUrl -> openUrl(context, request.url)
                    is HostRequest.WriteClipboard -> copy(context, request.text)
                    is HostRequest.Toast -> Toast.makeText(context, request.message, Toast.LENGTH_LONG).show()
                    is HostRequest.PickPaths -> {
                        pendingPick = request.request.toLong()
                        picker.pick()
                    }
                    HostRequest.Quit -> onQuit()
                    is HostRequest.OpenPath,
                    is HostRequest.ShowNotification,
                    is HostRequest.TransferActive,
                    HostRequest.RequestNotificationPermission,
                    HostRequest.RequestAllFilesAccess,
                    HostRequest.BringToFront,
                    -> CorveneLog.i("host request not handled yet (M-A3): $request")
                }
            }
        }
    }
}

private fun openUrl(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        Toast.makeText(context, context.getString(R.string.plt_no_app_for_url, url), Toast.LENGTH_LONG).show()
    }
}

private fun copy(context: Context, text: String) {
    val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return
    clipboard.setPrimaryClip(ClipData.newPlainText("Corvene", text))
}
