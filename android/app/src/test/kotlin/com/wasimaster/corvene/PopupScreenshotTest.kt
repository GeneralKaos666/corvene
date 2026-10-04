package com.wasimaster.corvene

import android.app.Application
import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.BranchVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.KeyList
import com.wasimaster.corvene.ffi.gen.KeyValue
import com.wasimaster.corvene.ffi.gen.PopupVm
import com.wasimaster.corvene.ffi.gen.SyncActionVm
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Three representative dialogs (a choice, a danger confirmation, a list) in every style, light and dark, on a phone. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(application = Application::class, qualifiers = "w360dp-h640dp-xxhdpi")
class PopupScreenshotTest(private val style: DesignStyle, private val mode: ColorMode) {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(name: String, popup: PopupVm) {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) {
                PopupDialog(popup, Context, NoActions)
            }
        }
        compose.waitForIdle()
        captureScreenRoboImage("src/test/screenshots/popup_${name}_${style.key}_${mode.key}.png")
    }

    @Test
    fun stashAndSwitchBranch() = shoot("stash_and_switch", popup("StashAndSwitchBranch", mapOf("branch" to "feature-a")))

    @Test
    fun deleteBranch() = shoot("delete_branch", popup("DeleteBranch", mapOf("name" to "feature-a")))

    @Test
    fun discardChanges() = shoot(
        "discard_changes",
        popup("DiscardChanges", mapOf("all" to "false"), mapOf("paths" to listOf("README.md", "src/main.rs", "docs/guide.md"))),
    )

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}_{1}")
        fun matrix(): List<Array<Any>> = DesignStyle.entries.flatMap { style ->
            listOf(ColorMode.Light, ColorMode.Dark).map { arrayOf<Any>(style, it) }
        }

        private fun popup(kind: String, fields: Map<String, String>, lists: Map<String, List<String>> = emptyMap()) = PopupVm(
            kind,
            1u,
            fields.map { (k, v) -> KeyValue(k, v) },
            lists.map { (k, v) -> KeyList(k, v) },
        )

        private val Context = PopupContext(
            branches = BranchesVm(
                1u, "main", null, "main", emptyList(),
                listOf(BranchVm("main", false, true, "origin/main", null), BranchVm("feature-a", false, false, "origin/feature-a", null)),
                null, null, SyncActionVm.FETCH, null, null, null, 0u,
            ),
        )

        private object NoActions : PopupActions {
            override fun close() = Unit

            override fun discardChanges(repo: ULong, paths: List<String>) = Unit

            override fun deleteBranch(repo: ULong, name: String, includeRemote: Boolean) = Unit

            override fun renameBranch(repo: ULong, old: String, new: String) = Unit

            override fun createBranch(repo: ULong, name: String, startPoint: String?) = Unit

            override fun createTag(repo: ULong, name: String, sha: String, message: String) = Unit

            override fun push(repo: ULong, force: Boolean) = Unit

            override fun pull(repo: ULong) = Unit

            override fun fetch(repo: ULong) = Unit

            override fun checkoutBranch(repo: ULong, name: String, strategy: String?) = Unit

            override fun dropStash(repo: ULong) = Unit

            override fun stashAllChanges(repo: ULong) = Unit

            override fun checkoutCommit(repo: ULong, sha: String) = Unit

            override fun resetToCommit(repo: ULong, sha: String) = Unit

            override fun undoCommit(repo: ULong) = Unit

            override fun commit(repo: ULong, summary: String, description: String) = Unit

            override fun removeRepository(repo: ULong) = Unit

            override fun mergeBranch(repo: ULong, branch: String, squash: Boolean) = Unit

            override fun submitGenericAuth(username: String, password: String) = Unit

            override fun retry() = Unit

            override fun publishRepository(
                repo: ULong,
                name: String,
                description: String,
                private: Boolean,
                endpoint: String,
                org: String?,
            ) =
                Unit
        }
    }
}
