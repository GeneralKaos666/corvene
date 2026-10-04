package com.wasimaster.corvene.platform

import android.content.Context
import android.content.pm.ProviderInfo
import android.provider.DocumentsContract
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import java.io.File

/** A picked folder becomes a path: own provider, shared storage with access, an imported copy, or a refusal. */
@RunWith(AndroidJUnit4::class)
class FolderResolverTest {

    @get:Rule
    val temp = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun shared() {
        FakeTreeProvider.root = temp.newFolder("shared")
        val info = ProviderInfo().apply {
            authority = FolderResolver.EXTERNAL_STORAGE
            exported = true
            grantUriPermissions = true
            readPermission = android.Manifest.permission.MANAGE_DOCUMENTS
            writePermission = android.Manifest.permission.MANAGE_DOCUMENTS
        }
        Robolectric.buildContentProvider(FakeTreeProvider::class.java).create(info)
    }

    private fun repository(name: String): File = File(FakeTreeProvider.root, name).apply {
        File(this, ".git/objects/ab").mkdirs()
        File(this, ".git/HEAD").writeText("ref: refs/heads/main\n")
        File(this, ".git/objects/ab/cdef").writeBytes(byteArrayOf(0, 1, 2, 3))
        File(this, "src").mkdirs()
        File(this, "src/main.rs").writeText("fn main() {}\n")
    }

    private fun tree(id: String) = DocumentsContract.buildTreeDocumentUri(FolderResolver.EXTERNAL_STORAGE, id)

    @Test
    fun `a folder of the own provider is its file`() {
        val tree = DocumentsContract.buildTreeDocumentUri(CorveneDocumentsProvider.authority(context), "repositories/demo")
        val picked = FolderResolver.resolve(context, tree, allFilesAccess = false)
        assertEquals(Picked.Path(File(context.filesDir, "repositories/demo").path), picked)
    }

    @Test
    fun `shared storage with all files access is used in place`() {
        val picked = FolderResolver.resolve(context, tree("primary:Corvene/demo2"), allFilesAccess = true) as Picked.Path
        assertTrue(picked.path, picked.path.endsWith("/Corvene/demo2"))
    }

    @Test
    fun `without access a repository is imported`() {
        repository("demo2")
        val importing = mutableListOf<String>()
        val picked = FolderResolver.resolve(context, tree("primary:demo2"), allFilesAccess = false) { importing += it } as Picked.Path
        val copy = File(picked.path)
        assertEquals(File(context.filesDir, "repositories/demo2"), copy)
        assertEquals(listOf("demo2"), importing)
        assertEquals("ref: refs/heads/main\n", File(copy, ".git/HEAD").readText())
        assertEquals(4, File(copy, ".git/objects/ab/cdef").readBytes().size)
        assertEquals("fn main() {}\n", File(copy, "src/main.rs").readText())
    }

    @Test
    fun `a second import of the same name gets a suffix`() {
        repository("demo2")
        FolderResolver.resolve(context, tree("primary:demo2"), allFilesAccess = false)
        val second = FolderResolver.resolve(context, tree("primary:demo2"), allFilesAccess = false) as Picked.Path
        assertEquals("demo2-2", File(second.path).name)
    }

    @Test
    fun `a folder without git is refused`() {
        File(FakeTreeProvider.root, "photos").mkdirs()
        val picked = FolderResolver.resolve(context, tree("primary:photos"), allFilesAccess = false)
        assertTrue(picked is Picked.Failed)
        assertTrue(!File(context.filesDir, "repositories/photos").exists())
    }

    @Test
    fun `a destination is never imported`() {
        repository("demo2")
        val picked = FolderResolver.resolve(context, tree("primary:demo2"), import = false, allFilesAccess = false)
        assertTrue(picked is Picked.Failed)
        assertTrue(!File(context.filesDir, "repositories/demo2").exists())
    }
}
