package com.wasimaster.corvene.ffi

/**
 * What the engine asks of the Android side (the `HostEvents` callbacks),
 * as values the activity handles in order. Delivered through [Core.hostRequests].
 */
sealed interface HostRequest {
    data class OpenUrl(val url: String) : HostRequest

    /** Answer with `Corvene.pathsPicked(request, paths)`; `null` paths = cancelled. */
    data class PickPaths(val request: ULong, val directories: Boolean, val multiple: Boolean, val prompt: String?) : HostRequest

    data class WriteClipboard(val text: String) : HostRequest

    data class OpenPath(val path: String, val reveal: Boolean) : HostRequest

    data class Toast(val message: String) : HostRequest

    data class ShowNotification(val identifier: String, val title: String, val body: String, val payload: String) : HostRequest

    data object RequestNotificationPermission : HostRequest

    data object RequestAllFilesAccess : HostRequest

    data class TransferActive(val active: Boolean) : HostRequest

    data object BringToFront : HostRequest

    data object Quit : HostRequest

    /** Start the process again (a flag that needs a restart changed). */
    data object Relaunch : HostRequest

    /** The share sheet for a file. */
    data class SharePath(val path: String) : HostRequest

    /** A Termux session in [dir]. */
    data class OpenTermux(val dir: String) : HostRequest

    /** [program] with [arguments] in a new Termux session in [dir]. */
    data class RunTermux(val program: String, val arguments: List<String>, val dir: String) : HostRequest

    /** Opens [path] in the application [component] ("package/class") names, at [line] when it can. */
    data class ViewPathWith(val path: String, val component: String, val line: Int?) : HostRequest
}
