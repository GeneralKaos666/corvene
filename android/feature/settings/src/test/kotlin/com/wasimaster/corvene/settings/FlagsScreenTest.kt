package com.wasimaster.corvene.settings

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.FlagsVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

/** Settings › Flags: the filters, each control's dispatch, Reset, presets, Reset all, the restart bar. */
@RunWith(AndroidJUnit4::class)
@Config(qualifiers = "w360dp-h2400dp-xxhdpi")
class FlagsScreenTest {

    @get:Rule
    val compose = createComposeRule()

    /** The text field inside a PrimerTextField (the tag sits on its column). */
    private fun field(tag: String) = compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(tag)))

    private val calls = mutableListOf<String>()
    private val actions = object : FlagActions {
        override fun set(slug: String, value: String) {
            calls += "set $slug=$value"
        }

        override fun reset(slug: String) {
            calls += "reset $slug"
        }

        override fun applyPreset(slug: String) {
            calls += "preset $slug"
        }

        override fun resetAll() {
            calls += "resetAll"
        }

        override fun relaunch() {
            calls += "relaunch"
        }
    }

    private fun show(flags: FlagsVm = SampleFlags, errors: Map<String, String> = emptyMap()) {
        compose.setContent {
            var view by remember { mutableStateOf(FlagsView()) }
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                FlagsScreen(flags, view, { view = it }, errors, actions)
            }
        }
    }

    private fun row(slug: String) = compose.onNodeWithTag("$TAG_FLAG$slug")

    private fun scrollTo(tag: String) {
        compose.onNodeWithTag(TAG_FLAGS).performScrollToNode(hasTestTag(tag))
    }

    @Test
    fun `bug fixes stay hidden until asked for`() {
        show()
        row("diff-check-marks-fix").assertDoesNotExist()
        compose.onNodeWithTag(TAG_FLAGS_BUG_FIXES).performClick()
        scrollTo("${TAG_FLAG}diff-check-marks-fix")
        row("diff-check-marks-fix").assertExists()
    }

    @Test
    fun `on and off narrow the list`() {
        show()
        compose.onNodeWithText("On").performClick()
        row("renamed-files-filter").assertExists()
        row("commit-template").assertDoesNotExist()
        compose.onNodeWithText("Off").performClick()
        row("renamed-files-filter").assertDoesNotExist()
        scrollTo("${TAG_FLAG}commit-template")
        row("commit-template").assertExists()
    }

    @Test
    fun `the search matches the ident and the title`() {
        show()
        field(TAG_FLAGS_QUERY).performTextReplacement("402-")
        scrollTo("${TAG_FLAG}history-page-size")
        row("history-page-size").assertExists()
        row("renamed-files-filter").assertDoesNotExist()
    }

    @Test
    fun `a toggle sets and an overridden flag resets`() {
        show()
        compose.onNodeWithTag("${TAG_FLAG_CONTROL}renamed-files-filter").performClick()
        compose.onNodeWithTag("${TAG_FLAG_DOT}renamed-files-filter", useUnmergedTree = true).assertExists()
        compose.onNodeWithTag("${TAG_FLAG_RESET}renamed-files-filter").performClick()
        assertEquals(listOf("set renamed-files-filter=false", "reset renamed-files-filter"), calls)
    }

    @Test
    fun `a select picks an option and a number steps within its range`() {
        show()
        scrollTo("${TAG_FLAG}design-style")
        compose.onNodeWithTag("${TAG_FLAG_CONTROL}design-style").performClick()
        compose.onNodeWithTag("${TAG_FLAG_OPTION}design-style:material").performClick()
        scrollTo("${TAG_FLAG}history-page-size")
        compose.onNodeWithTag("${TAG_FLAG_INCREASE}history-page-size").performClick()
        compose.onNodeWithTag("${TAG_FLAG_DECREASE}history-page-size").performClick()
        assertEquals(
            listOf("set design-style=material", "set history-page-size=101", "set history-page-size=99"),
            calls,
        )
    }

    @Test
    fun `text applies and the engine's refusal shows under it`() {
        show(errors = mapOf("commit-template" to "not a template"))
        scrollTo("${TAG_FLAG}commit-template")
        field("${TAG_FLAG_CONTROL}commit-template").performTextReplacement("feat: ")
        compose.onNodeWithTag("${TAG_FLAG_APPLY}commit-template").performClick()
        compose.onNodeWithTag("${TAG_FLAG_ERROR}commit-template").assertExists()
        assertEquals(listOf("set commit-template=feat: "), calls)
    }

    @Test
    fun `an unavailable flag takes no input`() {
        show()
        scrollTo("${TAG_FLAG}tree-sitter-highlighting")
        compose.onNodeWithTag("${TAG_FLAG_CONTROL}tree-sitter-highlighting").performClick()
        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun `reset all and a preset ask first`() {
        show()
        compose.onNodeWithTag(TAG_FLAGS_RESET_ALL).performClick()
        compose.onNodeWithTag(TAG_FLAGS_CONFIRM).performClick()
        compose.onNodeWithTag(TAG_FLAGS_PRESET).performClick()
        compose.onNodeWithTag("${TAG_FLAGS_PRESET_ITEM}github-desktop").performClick()
        compose.onNodeWithTag(TAG_FLAGS_CONFIRM).performClick()
        assertEquals(listOf("resetAll", "preset github-desktop"), calls)
    }

    @Test
    fun `no restart bar without a pending flag`() {
        show()
        compose.onNodeWithTag(TAG_FLAGS_RESTART).assertDoesNotExist()
    }

    @Test
    fun `the restart bar relaunches`() {
        val pending = SampleFlags.copy(flags = SampleFlags.flags.map { if (it.slug == "design-style") it.copy(restartPending = true) else it })
        show(pending)
        compose.onNodeWithTag(TAG_FLAGS_RELAUNCH).performClick()
        assertEquals(listOf("relaunch"), calls)
    }
}
