package com.wasimaster.corvene.settings

import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.RepositorySettingsVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

/** Repository settings: Save sends only what changed, per tab. */
@RunWith(AndroidJUnit4::class)
@Config(qualifiers = "w360dp-h800dp-xxhdpi")
class RepositorySettingsTest {

    @get:Rule
    val compose = createComposeRule()

    /** The text field inside a PrimerTextField (the tag sits on its column). */
    private fun field(tag: String) = compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(tag)))

    private val saved = mutableListOf<RepositorySettingsSave>()

    private fun show(settings: RepositorySettingsVm? = SampleRepositorySettings, tab: RepositorySettingsTab = RepositorySettingsTab.Remote) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                RepositorySettingsDialog(settings, tab, onSave = { saved += it }, onDismissRequest = {})
            }
        }
    }

    @Test
    fun `nothing changed saves nothing`() {
        show()
        compose.onNodeWithTag(TAG_REPO_SAVE).assertIsNotEnabled()
    }

    @Test
    fun `the remote url is saved with its remote`() {
        show()
        field(TAG_REPO_URL).performTextReplacement("git@github.com:wasi-master/corvene.git")
        compose.onNodeWithTag(TAG_REPO_SAVE).performClick()
        assertEquals(listOf(RepositorySettingsSave(remoteName = "origin", remoteUrl = "git@github.com:wasi-master/corvene.git")), saved)
    }

    @Test
    fun `the gitignore text is saved alone`() {
        show(tab = RepositorySettingsTab.IgnoredFiles)
        field(TAG_REPO_GITIGNORE).performTextReplacement("target/\n*.log\nbuild/\n")
        compose.onNodeWithTag(TAG_REPO_SAVE).performClick()
        assertEquals(listOf(RepositorySettingsSave(gitignore = "target/\n*.log\nbuild/\n")), saved)
    }

    @Test
    fun `a local identity and autocrlf`() {
        show(tab = RepositorySettingsTab.GitConfig)
        compose.onNodeWithTag(TAG_REPO_LOCAL).performClick()
        field(TAG_REPO_EMAIL).performTextReplacement("work@example.com")
        compose.onNodeWithTag(TAG_REPO_AUTOCRLF).performClick()
        compose.onNodeWithTag(TAG_REPO_SAVE).performClick()
        assertEquals(
            listOf(
                RepositorySettingsSave(gitConfigLocation = "local", name = "Wasi Master", email = "work@example.com", autocrlf = "true"),
            ),
            saved,
        )
    }

    @Test
    fun `back to the global identity removes the local one`() {
        show(SampleRepositorySettings.copy(localName = "Work", localEmail = "work@example.com"), RepositorySettingsTab.GitConfig)
        compose.onNodeWithTag(TAG_REPO_GLOBAL).performClick()
        compose.onNodeWithTag(TAG_REPO_SAVE).performClick()
        assertEquals(listOf(RepositorySettingsSave(gitConfigLocation = "global")), saved)
    }

    @Test
    fun `no remote and loading`() {
        show(SampleRepositorySettings.copy(remoteName = null, remoteUrl = null))
        compose.onNodeWithTag(TAG_REPO_NO_REMOTE).assertExists()
    }

    @Test
    fun `the popup's tab names map to tabs`() {
        assertEquals(RepositorySettingsTab.IgnoredFiles, RepositorySettingsTab.fromPopup("IgnoredFiles"))
        assertEquals(RepositorySettingsTab.GitConfig, RepositorySettingsTab.fromPopup("git"))
        assertEquals(RepositorySettingsTab.Remote, RepositorySettingsTab.fromPopup(null))
    }

    @Test
    fun `while loading it says so`() {
        show(settings = null)
        compose.onNodeWithText("Reading", substring = true).assertExists()
    }
}
