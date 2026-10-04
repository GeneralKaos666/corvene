package com.wasimaster.corvene.platform

import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import java.io.File
import java.io.FileOutputStream
import java.io.IOException

/** What a picker returned, as the engine needs it: a path, or why there is none. */
sealed interface Picked {
    data class Path(val path: String) : Picked

    /** [message] is for the user (a toast). */
    data class Failed(val message: String) : Picked
}

/**
 * The path behind a document tree the user picked (CorveneActivity.resolvePickedFolder
 * of the GPUI app):
 *
 * 1. Corvene's own documents provider: the file under `files/`;
 * 2. `primary:` on shared storage with "All files access": `/storage/emulated/0/<path>`;
 * 3. anything else that holds a `.git`: imported, the whole tree copied into
 *    `files/repositories/<name>` (`<name>-2`, … when taken), and that copy's path;
 * 4. else "not a Git repository" (or, where all-files access could be asked
 *    for, that it is needed to use the folder in place).
 *
 * Without [import] (a destination: where to clone or create) step 3 is
 * skipped: only folders git can write in place are answered.
 *
 * Blocking (the import copies files): call it off the main thread.
 */
object FolderResolver {
    const val EXTERNAL_STORAGE = "com.android.externalstorage.documents"
    private const val PRIMARY = "primary:"
    private const val COPY_BUFFER = 1 shl 16

    fun resolve(
        context: Context,
        tree: Uri,
        import: Boolean = true,
        allFilesAccess: Boolean = hasAllFilesAccess(context),
        onImporting: (String) -> Unit = {},
    ): Picked {
        val documentId = DocumentsContract.getTreeDocumentId(tree)
        val authority = tree.authority
        if (authority == CorveneDocumentsProvider.authority(context)) {
            val file = CorveneDocumentsProvider.fileOf(context, documentId)
                ?: return Picked.Failed(context.getString(R.string.plt_pick_unreadable))
            return Picked.Path(file.path)
        }
        if (authority == EXTERNAL_STORAGE && documentId.startsWith(PRIMARY) && allFilesAccess) {
            @Suppress("DEPRECATION") // the path is what git needs; MANAGE_EXTERNAL_STORAGE grants it
            val root = Environment.getExternalStorageDirectory()
            return Picked.Path(File(root, documentId.removePrefix(PRIMARY)).path)
        }
        if (!import) {
            val reason = if (canRequestAllFilesAccess(context)) R.string.plt_pick_needs_all_files else R.string.plt_pick_own_storage_only
            return Picked.Failed(context.getString(reason))
        }
        return try {
            importTree(context, tree, documentId, onImporting)
        } catch (error: IOException) {
            Picked.Failed(context.getString(R.string.plt_import_failed, error.message.orEmpty()))
        } catch (error: SecurityException) {
            Picked.Failed(context.getString(R.string.plt_import_failed, error.message.orEmpty()))
        }
    }

    private fun importTree(context: Context, tree: Uri, documentId: String, onImporting: (String) -> Unit): Picked {
        val children = children(context, tree, documentId)
        if (children.none { it.name == ".git" }) {
            val reason = if (tree.authority == EXTERNAL_STORAGE && canRequestAllFilesAccess(context)) {
                R.string.plt_pick_needs_all_files
            } else {
                R.string.plt_pick_not_repository
            }
            return Picked.Failed(context.getString(reason))
        }
        val name = displayName(context, DocumentsContract.buildDocumentUriUsingTree(tree, documentId))
        val base = CorveneDocumentsProvider.repositoriesDirectory(context)
        var target = File(base, name)
        var n = 2
        while (target.exists()) target = File(base, "$name-${n++}")
        onImporting(target.name)
        try {
            copyTree(context, tree, documentId, target)
        } catch (error: IOException) {
            target.deleteRecursively()
            throw error
        }
        return Picked.Path(target.path)
    }

    private data class Child(val id: String, val name: String, val directory: Boolean)

    /** One cursor per folder (DocumentFile would query the provider again for each name and type). */
    private fun children(context: Context, tree: Uri, documentId: String): List<Child> {
        val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, documentId)
        val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE)
        val result = mutableListOf<Child>()
        context.contentResolver.query(uri, columns, null as Bundle?, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                val name = cursor.getString(1) ?: continue
                if ('/' in name || name == "." || name == "..") continue
                result += Child(cursor.getString(0), name, cursor.getString(2) == Document.MIME_TYPE_DIR)
            }
        } ?: throw IOException("the folder cannot be listed")
        return result
    }

    private fun displayName(context: Context, document: Uri): String {
        context.contentResolver.query(document, arrayOf(Document.COLUMN_DISPLAY_NAME), null as Bundle?, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0)?.let { return it.replace('/', '_') }
        }
        return "repository"
    }

    private fun copyTree(context: Context, tree: Uri, documentId: String, target: File) {
        if (!target.mkdirs() && !target.isDirectory) throw IOException("could not create $target")
        for (child in children(context, tree, documentId)) {
            val file = File(target, child.name)
            if (child.directory) {
                copyTree(context, tree, child.id, file)
                continue
            }
            val source = DocumentsContract.buildDocumentUriUsingTree(tree, child.id)
            val input = context.contentResolver.openInputStream(source) ?: throw IOException("could not read ${child.name}")
            input.use { FileOutputStream(file).use { output -> it.copyTo(output, COPY_BUFFER) } }
        }
    }

    /** "All files access" (Android 11+), or the legacy storage permission before it. */
    fun hasAllFilesAccess(context: Context): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            context.checkSelfPermission(android.Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        }

    /** This build declares MANAGE_EXTERNAL_STORAGE (the foss flavour) on Android 11+. */
    fun canRequestAllFilesAccess(context: Context): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return false
        return try {
            val info = context.packageManager.getPackageInfo(context.packageName, PackageManager.GET_PERMISSIONS)
            info.requestedPermissions.orEmpty().contains(MANAGE_EXTERNAL_STORAGE)
        } catch (_: PackageManager.NameNotFoundException) {
            false
        }
    }

    private const val MANAGE_EXTERNAL_STORAGE = "android.permission.MANAGE_EXTERNAL_STORAGE"
}

/**
 * A single picked file (an SSH key, a patch) as a path: documents have none,
 * so it is copied to `cache/tmp/picked/file` (the previous copy goes), up to
 * [PICKED_FILE_LIMIT] bytes. Blocking.
 */
object FileCopier {
    const val PICKED_FILE_LIMIT = 16L shl 20

    fun copy(context: Context, document: Uri): Picked {
        val folder = File(File(context.cacheDir, "tmp"), "picked")
        folder.mkdirs()
        folder.listFiles()?.forEach { it.delete() }
        val copy = File(folder, "file")
        return try {
            val input = context.contentResolver.openInputStream(document) ?: throw IOException("the document cannot be read")
            input.use {
                FileOutputStream(copy).use { output ->
                    val buffer = ByteArray(1 shl 16)
                    var total = 0L
                    while (true) {
                        val read = it.read(buffer)
                        if (read <= 0) break
                        total += read
                        if (total > PICKED_FILE_LIMIT) throw IOException("the file is too large")
                        output.write(buffer, 0, read)
                    }
                }
            }
            Picked.Path(copy.path)
        } catch (error: IOException) {
            copy.delete()
            Picked.Failed(context.getString(R.string.plt_pick_file_failed, error.message.orEmpty()))
        } catch (error: SecurityException) {
            copy.delete()
            Picked.Failed(context.getString(R.string.plt_pick_file_failed, error.message.orEmpty()))
        }
    }
}
