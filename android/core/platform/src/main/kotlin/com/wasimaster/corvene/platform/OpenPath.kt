package com.wasimaster.corvene.platform

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.provider.DocumentsContract
import java.io.File
import java.io.FileInputStream
import java.io.IOException

/**
 * Files and folders handed to other applications (CorveneActivity.viewPath /
 * sharePath of the GPUI app). Other applications cannot read Corvene's
 * storage, so the intents carry a document of [CorveneDocumentsProvider] with
 * a grant. Each returns an error for the user, or null when the intent went.
 */
object OpenPath {
    private const val BINARY_PROBE = 8000

    /**
     * The engine's `open_path`: [reveal] shows the folder (the file's parent)
     * in the file manager; otherwise the file opens in the application the
     * user picks for its type.
     */
    fun open(context: Context, path: String, reveal: Boolean): String? {
        val file = File(path)
        val target = if (reveal && !file.isDirectory) file.parentFile ?: file else file
        return if (target.isDirectory) viewFolder(context, target) else viewFile(context, target)
    }

    private fun viewFolder(context: Context, folder: File): String? {
        val id = CorveneDocumentsProvider.idOf(context, folder) ?: return context.getString(R.string.plt_cannot_share, folder.path)
        // the file manager shows shared storage under its own provider
        val uri = if (id == CorveneDocumentsProvider.SHARED || id.startsWith("${CorveneDocumentsProvider.SHARED}/")) {
            DocumentsContract.buildDocumentUri(FolderResolver.EXTERNAL_STORAGE, "primary:" + id.substringAfter('/', ""))
        } else {
            DocumentsContract.buildDocumentUri(CorveneDocumentsProvider.authority(context), id)
        }
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        return start(context, intent, folder)
    }

    private fun viewFile(context: Context, file: File): String? {
        val uri = CorveneDocumentsProvider.documentUri(context, file) ?: return context.getString(R.string.plt_cannot_share, file.path)
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, mimeType(file))
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        return start(context, Intent.createChooser(intent, null), file)
    }

    /** ACTION_SEND through the share sheet, as a document the receiver may read. */
    fun share(context: Context, path: String): String? {
        val file = File(path)
        val uri = CorveneDocumentsProvider.documentUri(context, file)
        if (uri == null || !file.isFile) return context.getString(R.string.plt_cannot_share, path)
        val intent = Intent(Intent.ACTION_SEND)
            .setType(mimeType(file))
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        // the grant follows the clip data, not the extra
        intent.clipData = ClipData.newRawUri(file.name, uri)
        return start(context, Intent.createChooser(intent, null), file)
    }

    private fun start(context: Context, intent: Intent, file: File): String? = try {
        if (context !is android.app.Activity) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
        null
    } catch (_: ActivityNotFoundException) {
        context.getString(R.string.plt_no_app_for_file, file.name)
    }

    /** By extension; a file without a known one is text unless it has a NUL in its first 8000 bytes. */
    fun mimeType(file: File): String {
        val known = CorveneDocumentsProvider.mimeTypeOf(file)
        if (known != "application/octet-stream") return known
        val head = ByteArray(BINARY_PROBE)
        return try {
            val read = FileInputStream(file).use { it.read(head) }
            if ((0 until maxOf(read, 0)).any { head[it] == 0.toByte() }) known else "text/plain"
        } catch (_: IOException) {
            known
        }
    }
}
