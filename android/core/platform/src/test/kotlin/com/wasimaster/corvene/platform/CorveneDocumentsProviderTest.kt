package com.wasimaster.corvene.platform

import android.content.Context
import android.content.pm.ProviderInfo
import android.database.Cursor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import java.io.File
import java.io.FileNotFoundException

/** The provider's cursors and edits over files/repositories and files/home. */
@RunWith(AndroidJUnit4::class)
class CorveneDocumentsProviderTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private lateinit var provider: CorveneDocumentsProvider

    @Before
    fun create() {
        val info = ProviderInfo().apply {
            authority = CorveneDocumentsProvider.authority(context)
            exported = true
            grantUriPermissions = true
            readPermission = android.Manifest.permission.MANAGE_DOCUMENTS
            writePermission = android.Manifest.permission.MANAGE_DOCUMENTS
        }
        provider = Robolectric.buildContentProvider(CorveneDocumentsProvider::class.java).create(info).get()
        val repos = CorveneDocumentsProvider.repositoriesDirectory(context)
        File(repos, "demo/src").mkdirs()
        File(repos, "demo/README.md").writeText("# demo\n")
        File(repos, "demo/b.txt").writeText("b")
        File(repos, "demo/A.txt").writeText("a")
        File(CorveneDocumentsProvider.homeDirectory(context), ".gitconfig").writeText("[user]\n")
    }

    private fun Cursor.strings(column: String): List<String> = use {
        buildList { while (it.moveToNext()) add(it.getString(it.getColumnIndexOrThrow(column))) }
    }

    @Test
    fun `two roots, repositories and the settings`() {
        val ids = provider.queryRoots(null).strings(Root.COLUMN_ROOT_ID)
        assertEquals(listOf("repositories", "home"), ids)
        val titles = provider.queryRoots(null).strings(Root.COLUMN_TITLE)
        assertEquals(listOf("Corvene", "Corvene settings"), titles)
    }

    @Test
    fun `children are folders first, then by name`() {
        val names = provider.queryChildDocuments("repositories/demo", null, null as String?).strings(Document.COLUMN_DISPLAY_NAME)
        assertEquals(listOf("src", "A.txt", "b.txt", "README.md"), names)
        val types = provider.queryChildDocuments("repositories/demo", null, null as String?).strings(Document.COLUMN_MIME_TYPE)
        assertEquals(Document.MIME_TYPE_DIR, types.first())
    }

    @Test
    fun `dot files in the settings root are shown without the dot`() {
        val cursor = provider.queryChildDocuments("home", null, null as String?)
        assertEquals(listOf("home/%2Egitconfig"), provider.queryChildDocuments("home", null, null as String?).strings(Document.COLUMN_DOCUMENT_ID))
        assertEquals(listOf("gitconfig"), cursor.strings(Document.COLUMN_DISPLAY_NAME))
        assertEquals("text/plain", provider.getDocumentType("home/%2Egitconfig"))
    }

    @Test
    fun `create, rename and delete`() {
        val created = provider.createDocument("repositories/demo", "text/plain", "new.txt")
        assertEquals("repositories/demo/new.txt", created)
        assertTrue(File(context.filesDir, "repositories/demo/new.txt").isFile)
        val folder = provider.createDocument("repositories/demo", Document.MIME_TYPE_DIR, "docs")
        assertTrue(File(context.filesDir, "repositories/demo/docs").isDirectory)
        val renamed = provider.renameDocument(created, "renamed.txt")
        assertEquals("repositories/demo/renamed.txt", renamed)
        assertFalse(File(context.filesDir, "repositories/demo/new.txt").exists())
        provider.deleteDocument(folder)
        assertFalse(File(context.filesDir, "repositories/demo/docs").exists())
    }

    @Test(expected = FileNotFoundException::class)
    fun `a root cannot be deleted`() = provider.deleteDocument("repositories")

    @Test(expected = FileNotFoundException::class)
    fun `names with a slash are refused`() {
        provider.createDocument("repositories/demo", "text/plain", "../escape")
    }

    @Test
    fun `ids that leave their folder name nothing`() {
        assertNull(CorveneDocumentsProvider.fileOf(context, "repositories/../home/.gitconfig"))
        assertNull(CorveneDocumentsProvider.fileOf(context, "elsewhere/x"))
        assertEquals("repositories/demo/README.md", CorveneDocumentsProvider.idOf(context, File(context.filesDir, "repositories/demo/README.md")))
    }

    @Test
    fun `the path to a document runs from its root`() {
        val path = provider.findDocumentPath(null, "repositories/demo/src")
        assertEquals("repositories", path.rootId)
        assertEquals(listOf("repositories", "repositories/demo", "repositories/demo/src"), path.path)
    }
}
