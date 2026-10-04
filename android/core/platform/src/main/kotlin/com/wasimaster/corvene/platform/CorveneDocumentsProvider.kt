package com.wasimaster.corvene.platform

import android.content.Context
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.CancellationSignal
import android.os.Environment
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import android.webkit.MimeTypeMap
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.util.Locale

/**
 * Shows Corvene's repositories (`files/repositories` in app-private storage)
 * to other applications through the Storage Access Framework, so an editor or
 * a file manager can open and change the files of a clone (Kotlin port of the
 * GPUI app's CorveneDocumentsProvider.java).
 *
 * Document ids: `repositories` for the folder and `repositories/<path>` below
 * it; a second root `home` ("Corvene settings": git's `.gitconfig` and `.ssh`,
 * leading dots spelled [DOT] so file managers do not hide them). Two more
 * prefixes are never listed as roots: `shared/…` (shared storage, where
 * repositories opened in place live) and `tmp/…` (temporary copies handed to
 * a viewer); [openPath] uses them for its view intents.
 */
class CorveneDocumentsProvider : DocumentsProvider() {

    private val appContext: Context get() = requireNotNull(context)

    override fun onCreate(): Boolean = true

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val result = MatrixCursor(projection ?: ROOT_COLUMNS)
        addRoot(result, REPOSITORIES, repositoriesDirectory(appContext), R.string.plt_documents_title, R.string.plt_documents_summary)
        addRoot(result, HOME, homeDirectory(appContext), R.string.plt_home_title, R.string.plt_home_summary)
        return result
    }

    private fun addRoot(result: MatrixCursor, id: String, base: File, title: Int, summary: Int) {
        result.newRow().apply {
            add(Root.COLUMN_ROOT_ID, id)
            add(Root.COLUMN_DOCUMENT_ID, id)
            add(Root.COLUMN_TITLE, appContext.getString(title))
            add(Root.COLUMN_SUMMARY, appContext.getString(summary))
            add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE or Root.FLAG_SUPPORTS_IS_CHILD or Root.FLAG_LOCAL_ONLY)
            add(Root.COLUMN_MIME_TYPES, "*/*")
            add(Root.COLUMN_AVAILABLE_BYTES, base.freeSpace)
            add(Root.COLUMN_ICON, appContext.applicationInfo.icon)
        }
    }

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor {
        val result = MatrixCursor(projection ?: DOCUMENT_COLUMNS)
        addRow(result, fileFor(documentId))
        return result
    }

    /** Folders first, then by name (case-insensitive), whatever order the disk lists them in. */
    override fun queryChildDocuments(parentDocumentId: String, projection: Array<out String>?, sortOrder: String?): Cursor {
        val result = MatrixCursor(projection ?: DOCUMENT_COLUMNS)
        fileFor(parentDocumentId).listFiles().orEmpty()
            .sortedWith(compareBy<File> { !it.isDirectory }.thenBy { it.name.lowercase(Locale.ROOT) })
            .forEach { addRow(result, it) }
        return result
    }

    private fun addRow(result: MatrixCursor, file: File) {
        var flags = 0
        if (file.isDirectory) {
            if (file.canWrite()) flags = flags or Document.FLAG_DIR_SUPPORTS_CREATE
        } else if (file.canWrite()) {
            flags = flags or Document.FLAG_SUPPORTS_WRITE
        }
        val parent = file.parentFile
        val id = idOf(appContext, file) ?: REPOSITORIES
        // what the settings root is for is .gitconfig and .ssh: shown without
        // their dot, and they keep their names (no renaming)
        val dotted = parent != null && parent == homeDirectory(appContext) && file.name.startsWith(".") && file.name.length > 1
        if (parent != null && parent.canWrite() && !isTop(id)) {
            flags = flags or Document.FLAG_SUPPORTS_DELETE
            if (!dotted) flags = flags or Document.FLAG_SUPPORTS_RENAME
        }
        result.newRow().apply {
            add(Document.COLUMN_DOCUMENT_ID, id)
            add(
                Document.COLUMN_DISPLAY_NAME,
                when {
                    id == REPOSITORIES -> appContext.getString(R.string.plt_documents_title)
                    id == HOME -> appContext.getString(R.string.plt_home_title)
                    dotted -> file.name.substring(1)
                    else -> file.name
                },
            )
            add(Document.COLUMN_SIZE, file.length())
            add(Document.COLUMN_MIME_TYPE, mimeTypeOf(file))
            add(Document.COLUMN_LAST_MODIFIED, file.lastModified())
            add(Document.COLUMN_FLAGS, flags)
        }
    }

    override fun getDocumentType(documentId: String): String = mimeTypeOf(fileFor(documentId))

    override fun isChildDocument(parentDocumentId: String, documentId: String): Boolean =
        documentId == parentDocumentId || documentId.startsWith("$parentDocumentId/")

    /** The documents from the root down to [childDocumentId]: lets a file manager open at a folder it is sent to. */
    override fun findDocumentPath(parentDocumentId: String?, childDocumentId: String): DocumentsContract.Path {
        fileFor(childDocumentId)
        val top = parentDocumentId ?: if (childDocumentId == HOME || childDocumentId.startsWith("$HOME/")) HOME else REPOSITORIES
        if (!isChildDocument(top, childDocumentId)) throw FileNotFoundException(childDocumentId)
        val path = ArrayDeque<String>()
        var id = childDocumentId
        while (true) {
            path.addFirst(id)
            if (id == top) break
            id = id.substring(0, id.lastIndexOf('/'))
        }
        return DocumentsContract.Path(if (parentDocumentId == null) top else null, path.toList())
    }

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor =
        ParcelFileDescriptor.open(fileFor(documentId), ParcelFileDescriptor.parseMode(mode))

    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String {
        val parent = fileFor(parentDocumentId)
        if (!isPlainName(displayName)) throw FileNotFoundException(displayName)
        val file = File(parent, displayName)
        val created = try {
            if (mimeType == Document.MIME_TYPE_DIR) file.mkdir() else file.createNewFile()
        } catch (error: IOException) {
            throw FileNotFoundException("could not create $displayName").apply { initCause(error) }
        }
        if (!created) throw FileNotFoundException("could not create $displayName")
        return idOf(appContext, file) ?: throw FileNotFoundException(displayName)
    }

    override fun deleteDocument(documentId: String) {
        if (isTop(documentId) || !fileFor(documentId).deleteRecursively()) {
            throw FileNotFoundException("could not delete $documentId")
        }
    }

    override fun renameDocument(documentId: String, displayName: String): String {
        val file = fileFor(documentId)
        val dotted = file.parentFile == homeDirectory(appContext) && file.name.startsWith(".")
        if (isTop(documentId) || dotted || !isPlainName(displayName)) throw FileNotFoundException(displayName)
        val renamed = File(file.parentFile, displayName)
        if (renamed.exists() || !file.renameTo(renamed)) throw FileNotFoundException("could not rename $documentId")
        return idOf(appContext, renamed) ?: throw FileNotFoundException(displayName)
    }

    /** The file of a document id, refusing ids that leave their folder or do not exist. */
    private fun fileFor(documentId: String): File {
        val file = fileOf(appContext, documentId) ?: throw FileNotFoundException(documentId)
        if (!file.exists()) throw FileNotFoundException(documentId)
        return file
    }

    companion object {
        /** The authority, `${applicationId}.documents` in the manifest. */
        fun authority(context: Context): String = "${context.packageName}.documents"

        const val REPOSITORIES = "repositories"
        const val HOME = "home"
        const val SHARED = "shared"
        const val TMP = "tmp"

        /** Stands for the leading dot of a name in the settings root's ids. */
        const val DOT = "%2E"

        private val ROOT_COLUMNS = arrayOf(
            Root.COLUMN_ROOT_ID, Root.COLUMN_MIME_TYPES, Root.COLUMN_FLAGS, Root.COLUMN_ICON,
            Root.COLUMN_TITLE, Root.COLUMN_SUMMARY, Root.COLUMN_DOCUMENT_ID, Root.COLUMN_AVAILABLE_BYTES,
        )
        private val DOCUMENT_COLUMNS = arrayOf(
            Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE, Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_LAST_MODIFIED, Document.COLUMN_FLAGS, Document.COLUMN_SIZE,
        )

        /** `files/repositories`, where clones and imports go; created on first use. */
        fun repositoriesDirectory(context: Context): File = File(context.filesDir, REPOSITORIES).apply { mkdirs() }

        /** `files/home`: git's HOME (.gitconfig, .ssh). */
        fun homeDirectory(context: Context): File = File(context.filesDir, HOME).apply { mkdirs() }

        private fun tmpDirectory(context: Context): File = File(context.cacheDir, TMP)

        @Suppress("DEPRECATION") // the path is what git and Termux need
        private fun sharedDirectory(): File = Environment.getExternalStorageDirectory()

        private fun bases(context: Context): List<Pair<String, File>> = listOf(
            REPOSITORIES to repositoriesDirectory(context),
            HOME to homeDirectory(context),
            TMP to tmpDirectory(context),
            SHARED to sharedDirectory(),
        )

        /** The document id of [file]; null when it is in none of the folders. */
        fun idOf(context: Context, file: File): String? {
            val path = file.absolutePath
            for ((id, base) in bases(context)) {
                val root = base.absolutePath
                if (path == root) return id
                if (path.startsWith(root + File.separator)) {
                    var relative = path.substring(root.length + 1)
                    // file managers hide names starting with a dot, and the
                    // settings root is there for .gitconfig and .ssh
                    if (id == HOME && relative.startsWith(".")) relative = DOT + relative.substring(1)
                    return "$id/$relative"
                }
            }
            return null
        }

        /** The file a document id names; null for a foreign id or one that leaves its folder. */
        fun fileOf(context: Context, documentId: String): File? {
            val prefix = documentId.substringBefore('/')
            val base = bases(context).firstOrNull { it.first == prefix }?.second ?: return null
            var relative = if (documentId == prefix) "" else documentId.substring(prefix.length + 1)
            if (prefix == HOME && relative.startsWith(DOT)) relative = "." + relative.substring(DOT.length)
            val file = if (relative.isEmpty()) base else File(base, relative)
            return try {
                val canonical = file.canonicalPath
                val root = base.canonicalPath
                file.takeIf { canonical == root || canonical.startsWith(root + File.separator) }
            } catch (_: IOException) {
                null
            }
        }

        /** The content URI of [file] in this provider; null when other applications cannot be given it. */
        fun documentUri(context: Context, file: File): Uri? =
            idOf(context, file)?.let { DocumentsContract.buildDocumentUri(authority(context), it) }

        private fun isTop(documentId: String) = '/' !in documentId

        private fun isPlainName(name: String) = name.isNotEmpty() && '/' !in name && name != "." && name != ".."

        /** By extension; dot-files without one (`.gitconfig`) are text. */
        fun mimeTypeOf(file: File): String {
            if (file.isDirectory) return Document.MIME_TYPE_DIR
            val name = file.name
            if (name.startsWith(".") && name.indexOf('.', 1) < 0) return "text/plain"
            val extension = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
            return MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension) ?: "application/octet-stream"
        }
    }
}
