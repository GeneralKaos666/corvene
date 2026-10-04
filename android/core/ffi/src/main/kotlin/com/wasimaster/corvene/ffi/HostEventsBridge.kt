package com.wasimaster.corvene.ffi

import com.wasimaster.corvene.ffi.gen.HostEvents

/**
 * The engine's callbacks, implemented in Kotlin. They arrive on the engine's
 * threads; nothing here touches a view: [onStateChanged] bumps a StateFlow and
 * everything else is queued as a [HostRequest] for the activity.
 */
internal class HostEventsBridge(
    private val onStateChanged: (Long) -> Unit,
    private val send: (HostRequest) -> Unit,
    /** The applications that open a text file, as "label\tpackage/class" (answered at once). */
    private val viewApps: () -> List<String> = { emptyList() },
    private val packageInstalled: (String) -> Boolean = { false },
) : HostEvents {
    override fun stateChanged(version: ULong) = onStateChanged(version.toLong())

    override fun openUrl(url: String) = send(HostRequest.OpenUrl(url))

    override fun pickPaths(request: ULong, directories: Boolean, multiple: Boolean, prompt: String?) =
        send(HostRequest.PickPaths(request, directories, multiple, prompt))

    override fun writeClipboard(text: String) = send(HostRequest.WriteClipboard(text))

    override fun openPath(path: String, reveal: Boolean) = send(HostRequest.OpenPath(path, reveal))

    override fun toast(message: String) = send(HostRequest.Toast(message))

    override fun showNotification(identifier: String, title: String, body: String, payload: String) =
        send(HostRequest.ShowNotification(identifier, title, body, payload))

    override fun requestNotificationPermission() = send(HostRequest.RequestNotificationPermission)

    override fun requestAllFilesAccess() = send(HostRequest.RequestAllFilesAccess)

    override fun transferActive(active: Boolean) = send(HostRequest.TransferActive(active))

    override fun bringToFront() = send(HostRequest.BringToFront)

    override fun quit() = send(HostRequest.Quit)

    override fun relaunch() = send(HostRequest.Relaunch)

    override fun sharePath(path: String) = send(HostRequest.SharePath(path))

    override fun openTermux(dir: String) = send(HostRequest.OpenTermux(dir))

    override fun runTermux(program: String, arguments: List<String>, dir: String) =
        send(HostRequest.RunTermux(program, arguments, dir))

    override fun viewPathWith(path: String, component: String, line: UInt?) =
        send(HostRequest.ViewPathWith(path, component, line?.toInt()))

    override fun viewApps(): List<String> = viewApps.invoke()

    override fun packageInstalled(`package`: String): Boolean = packageInstalled.invoke(`package`)
}
