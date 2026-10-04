package com.wasimaster.corvene.platform

import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsProvider
import java.io.File
import java.io.FileNotFoundException

/**
 * Shared storage as the system's external storage provider shows it, over a
 * temporary folder: ids are `primary:<relative path>`. What the picker hands
 * back for a folder the app cannot use in place.
 */
class FakeTreeProvider : DocumentsProvider() {
    override fun onCreate(): Boolean = true

    private fun fileOf(id: String): File {
        val relative = id.removePrefix("primary:")
        val file = if (relative.isEmpty()) root else File(root, relative)
        if (!file.exists()) throw FileNotFoundException(id)
        return file
    }

    private fun idOf(file: File): String = "primary:" + file.relativeTo(root).path

    private fun row(cursor: MatrixCursor, file: File) {
        cursor.newRow()
            .add(Document.COLUMN_DOCUMENT_ID, idOf(file))
            .add(Document.COLUMN_DISPLAY_NAME, file.name)
            .add(Document.COLUMN_MIME_TYPE, if (file.isDirectory) Document.MIME_TYPE_DIR else "application/octet-stream")
    }

    override fun queryRoots(projection: Array<out String>?): Cursor = MatrixCursor(arrayOf("root_id"))

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor =
        MatrixCursor(projection ?: COLUMNS).also { row(it, fileOf(documentId)) }

    override fun queryChildDocuments(parentDocumentId: String, projection: Array<out String>?, sortOrder: String?): Cursor =
        MatrixCursor(projection ?: COLUMNS).also { cursor -> fileOf(parentDocumentId).listFiles().orEmpty().forEach { row(cursor, it) } }

    override fun isChildDocument(parentDocumentId: String, documentId: String): Boolean {
        val parent = parentDocumentId.removePrefix("primary:")
        val child = documentId.removePrefix("primary:")
        return parent.isEmpty() || child == parent || child.startsWith("$parent/")
    }

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor =
        ParcelFileDescriptor.open(fileOf(documentId), ParcelFileDescriptor.MODE_READ_ONLY)

    companion object {
        lateinit var root: File
        private val COLUMNS = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE)
    }
}
