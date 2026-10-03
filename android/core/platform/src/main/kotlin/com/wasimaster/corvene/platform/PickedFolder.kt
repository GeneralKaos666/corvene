package com.wasimaster.corvene.platform

import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import java.io.File

/** A folder the system picker returned, as the engine needs it: a path. */
sealed interface PickedFolder {
    data class Path(val path: String) : PickedFolder

    /** [message] is a string resource saying why. */
    data class Unsupported(val message: Int) : PickedFolder
}

/**
 * The path behind a document tree the user picked (ported from the GPUI
 * app's CorveneActivity.resolvePickedFolder):
 *
 * - Corvene's own documents provider: the file under `files/` (M-A3 brings
 *   the provider back; the branch is ready for it),
 * - `primary:` on shared storage, when the app has all-files access,
 * - anything else: unsupported for now (M-A3 imports a copy of a Git repository).
 */
object FolderResolver {
    private const val EXTERNAL_STORAGE = "com.android.externalstorage.documents"
    private const val PRIMARY = "primary:"

    fun resolve(context: Context, tree: Uri): PickedFolder {
        val documentId = DocumentsContract.getTreeDocumentId(tree)
        val authority = tree.authority
        if (authority == "${context.packageName}.documents") {
            return PickedFolder.Path(File(File(context.filesDir, "repositories"), documentId.substringAfter('/', "")).path)
        }
        if (authority == EXTERNAL_STORAGE && documentId.startsWith(PRIMARY)) {
            if (!hasAllFilesAccess(context)) return PickedFolder.Unsupported(R.string.plt_pick_needs_all_files)
            @Suppress("DEPRECATION") // the path is what git needs; MANAGE_EXTERNAL_STORAGE grants it
            val root = Environment.getExternalStorageDirectory()
            return PickedFolder.Path(File(root, documentId.removePrefix(PRIMARY)).path)
        }
        val reason = if (authority == EXTERNAL_STORAGE) R.string.plt_pick_not_shared_storage else R.string.plt_pick_import_later
        return PickedFolder.Unsupported(reason)
    }

    /** "All files access" (Android 11+), or the legacy storage permission before it. */
    fun hasAllFilesAccess(context: Context): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            context.checkSelfPermission(android.Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
                android.content.pm.PackageManager.PERMISSION_GRANTED
        }
}
