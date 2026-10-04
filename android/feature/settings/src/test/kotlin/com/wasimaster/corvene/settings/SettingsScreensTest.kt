package com.wasimaster.corvene.settings

import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

/** Every Settings toggle dispatches `setSetting` with its engine key and the new value. */
@RunWith(AndroidJUnit4::class)
@Config(qualifiers = "w360dp-h2000dp-xxhdpi")
class SettingsScreensTest {

    @get:Rule
    val compose = createComposeRule()

    /** The text field inside a PrimerTextField (the tag sits on its column). */
    private fun field(tag: String) = compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(tag)))

    private val written = mutableListOf<Pair<String, String>>()
    private val writer = SettingWriter { key, value -> written += key to value }

    private fun show(content: @androidx.compose.runtime.Composable () -> Unit) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) { content() }
        }
    }

    private fun toggle(key: String) {
        compose.onNodeWithTag("$TAG_SETTING$key").performScrollTo().performClick()
    }

    @Test
    fun `prompts send each confirmation's key`() {
        show { PromptsScreen(SampleSettings, writer) }
        val keys = listOf(
            SettingKey.CONFIRM_REPOSITORY_REMOVAL,
            SettingKey.CONFIRM_DISCARD_CHANGES,
            SettingKey.CONFIRM_DISCARD_STASH,
            SettingKey.CONFIRM_CHECKOUT_COMMIT,
            SettingKey.CONFIRM_FORCE_PUSH,
            SettingKey.CONFIRM_UNDO_COMMIT,
            SettingKey.CONFIRM_COMMIT_FILTERED_CHANGES,
        )
        keys.forEach(::toggle)
        toggle(SettingKey.SHOW_COMMIT_LENGTH_WARNING)
        assertEquals(keys.map { it to "false" } + (SettingKey.SHOW_COMMIT_LENGTH_WARNING to "true"), written)
    }

    @Test
    fun `the branch switch strategy sends its value`() {
        show { PromptsScreen(SampleSettings, writer) }
        compose.onNodeWithTag("${TAG_STRATEGY}stash").performScrollTo().performClick()
        compose.onNodeWithTag("${TAG_STRATEGY}move").performScrollTo().performClick()
        assertEquals(
            listOf(SettingKey.UNCOMMITTED_CHANGES_STRATEGY to "stash", SettingKey.UNCOMMITTED_CHANGES_STRATEGY to "move"),
            written,
        )
    }

    @Test
    fun `accessibility and advanced send their keys`() {
        show {
            androidx.compose.foundation.layout.Column {
                AccessibilityScreen(SampleSettings, writer)
            }
        }
        toggle(SettingKey.UNDERLINE_LINKS)
        toggle(SettingKey.SHOW_DIFF_CHECK_MARKS)
        assertEquals(listOf(SettingKey.UNDERLINE_LINKS to "false", SettingKey.SHOW_DIFF_CHECK_MARKS to "false"), written)
    }

    @Test
    fun `advanced sends the indicators key and opens all files access`() {
        var opened = 0
        show { AdvancedScreen(SampleSettings.copy(repositoryIndicatorsEnabled = false), AllFilesAccess(false), writer, { opened++ }) }
        toggle(SettingKey.REPOSITORY_INDICATORS_ENABLED)
        compose.onNodeWithTag(TAG_ALL_FILES).performClick()
        assertEquals(listOf(SettingKey.REPOSITORY_INDICATORS_ENABLED to "true"), written)
        assertEquals(1, opened)
    }

    @Test
    fun `advanced without all files access hides the row`() {
        show { AdvancedScreen(SampleSettings, null, writer, {}) }
        compose.onNodeWithTag(TAG_ALL_FILES).assertDoesNotExist()
    }

    @Test
    fun `notifications toggle and the blocked hint`() {
        val enabled = mutableListOf<Boolean>()
        show { NotificationsScreen(SampleSettings, systemAllowed = false, onEnabled = { enabled += it }, onOpenSystemSettings = {}) }
        compose.onNodeWithTag(TAG_NOTIFICATIONS_BLOCKED).assertExists()
        toggle(SettingKey.NOTIFICATIONS_ENABLED)
        assertEquals(listOf(false), enabled)
    }

    @Test
    fun `integrations pick an editor by label and clear it`() {
        val editors = listOf(EditorApp("Acode", "com.foxdebug.acode/.MainActivity"), EditorApp("Markor", "net.gsantner.markor/.Main"))
        show { IntegrationsScreen(SampleSettings.copy(externalEditor = "Acode"), editors, termuxInstalled = true, writer = writer) }
        compose.onNodeWithTag("${TAG_EDITOR}Markor").performClick()
        compose.onNodeWithTag("${TAG_EDITOR}none").performClick()
        assertEquals(listOf(SettingKey.EXTERNAL_EDITOR to "Markor", SettingKey.EXTERNAL_EDITOR to ""), written)
        compose.onNodeWithText("Termux").assertExists()
    }

    @Test
    fun `viewApps lines parse into editors`() {
        val parsed = EditorApp.parse(listOf("Acode\tcom.foxdebug.acode/.Main", "broken", "Acode\tother/.Dup", "\tno.label/.X"))
        assertEquals(listOf(EditorApp("Acode", "com.foxdebug.acode/.Main")), parsed)
    }

    @Test
    fun `git saves the edited identity`() {
        val saved = mutableListOf<Pair<String, String>>()
        show { GitScreen("Wasi", "old@example.com", "main", onSave = { n, e -> saved += n to e }, onEditConfig = {}) }
        field(TAG_GIT_EMAIL).performTextReplacement("wasi@example.com ")
        compose.onNodeWithTag(TAG_GIT_SAVE).performClick()
        assertEquals(listOf("Wasi" to "wasi@example.com"), saved)
    }

    @Test
    fun `the sections list opens a section`() {
        val opened = mutableListOf<SettingsSection>()
        show { SettingsListScreen(onOpen = { opened += it }) }
        compose.onNodeWithTag("${TAG_SECTION}flags").performScrollTo().performClick()
        compose.onNodeWithTag("${TAG_SECTION}prompts").performScrollTo().performClick()
        assertEquals(listOf(SettingsSection.Flags, SettingsSection.Prompts), opened)
    }

    @Test
    fun `about shows the version and opens the licences`() {
        var licences = 0
        show { AboutScreen("0.1.0 (foss, release)", onLicenses = { licences++ }, onLink = {}) }
        compose.onNodeWithText("0.1.0 (foss, release)").assertExists()
        compose.onNodeWithTag(TAG_LICENSES).performClick()
        assertEquals(1, licences)
    }
}
