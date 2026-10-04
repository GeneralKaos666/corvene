package com.wasimaster.corvene.platform

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.HostRequest

/**
 * Handles the engine's [HostRequest]s while the activity is composed: URLs
 * (sign-in pages in a Custom Tab), the clipboard, toasts, the folder and file
 * pickers (each `pick_paths` answered exactly once with `pathsPicked`),
 * opening files and folders, notifications and their permission, all-files
 * access, the transfer service, bringing the app forward, quitting and
 * relaunching (a flag that needs a restart). Requests made while no activity
 * is up wait in [Core.hostRequests].
 */
@Composable
fun HostRequestHandler(core: Core, onQuit: () -> Unit, onRelaunch: () -> Unit) {
    val context = LocalContext.current
    // the engine's request id survives the activity being recreated under the picker
    var pendingPick by rememberSaveable { mutableStateOf<Long?>(null) }
    val answer: (String?) -> Unit = { path ->
        pendingPick?.let { request -> core.dispatch { pathsPicked(request.toULong(), path?.let(::listOf)) } }
        pendingPick = null
    }
    val folderPicker = rememberFolderPicker(onResult = answer)
    val filePicker = rememberFilePicker(onResult = answer)
    val notificationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        CorveneLog.i("notification permission: $granted")
    }
    LaunchedEffect(core) {
        core.hostRequests.collect { request ->
            CorveneTrace.section(CorveneTrace.HOST_REQUEST) {
                when (request) {
                    is HostRequest.OpenUrl ->
                        if (isSignInUrl(request.url)) openCustomTab(context, request.url) else openUrl(context, request.url)
                    is HostRequest.WriteClipboard -> writeClipboard(context, request.text)
                    is HostRequest.Toast -> toast(context, request.message)
                    is HostRequest.PickPaths -> {
                        // one picker at a time: an unanswered earlier request is cancelled
                        pendingPick?.let { old -> core.dispatch { pathsPicked(old.toULong(), null) } }
                        pendingPick = request.request.toLong()
                        if (request.directories) folderPicker.pick() else filePicker.pick()
                    }
                    is HostRequest.OpenPath -> OpenPath.open(context, request.path, request.reveal)?.let { toast(context, it) }
                    is HostRequest.ShowNotification ->
                        Notifications.show(context, request.identifier, request.title, request.body, request.payload)
                    is HostRequest.TransferActive -> TransferController.transferActive(context, request.active)
                    HostRequest.RequestNotificationPermission ->
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
                        }
                    HostRequest.RequestAllFilesAccess -> requestAllFilesAccess(context)
                    HostRequest.BringToFront -> bringToFront(context)
                    HostRequest.Quit -> onQuit()
                    HostRequest.Relaunch -> onRelaunch()
                    is HostRequest.SharePath -> OpenPath.share(context, request.path)?.let { toast(context, it) }
                    is HostRequest.OpenTermux ->
                        (context as? android.app.Activity)?.let { Termux.open(it, request.dir)?.let { m -> toast(context, m) } }
                    is HostRequest.RunTermux ->
                        (context as? android.app.Activity)?.let { Termux.open(it, request.dir)?.let { m -> toast(context, m) } }
                    is HostRequest.ViewPathWith ->
                        OpenPath.viewWith(context, request.path, request.component, request.line)?.let { toast(context, it) }
                }
            }
        }
    }
}
