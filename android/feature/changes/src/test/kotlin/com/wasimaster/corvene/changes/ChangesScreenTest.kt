package com.wasimaster.corvene.changes

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.ChangesVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

/** The Changes list against fixed view models (a portrait phone); the actions record what it sends. */
@RunWith(AndroidJUnit4::class)
@Config(qualifiers = "w360dp-h640dp")
class ChangesScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val sent = mutableListOf<String>()

    private val actions = object : ChangesActions {
        override fun select(path: String) {
            sent += "select $path"
        }

        override fun toggleIncluded(path: String) {
            sent += "toggle $path"
        }

        override fun discard(paths: List<String>) {
            sent += "discard $paths"
        }

        override fun toggleFilter(option: FilterOption) {
            sent += "filter ${option.key}"
        }

        override fun clearFilters() {
            sent += "clear"
        }

        override fun commit(summary: String, description: String) {
            sent += "commit $summary|$description"
        }

        override fun undo() {
            sent += "undo"
        }

        override fun restoreStash() {
            sent += "restore"
        }
    }

    private fun show(changes: ChangesVm, confirmDiscard: Boolean = true, style: DesignStyle = DesignStyle.GitHubMobile) {
        compose.setContent {
            CorveneTheme(style, ColorMode.Light, highContrast = false, dynamicColor = false) {
                ChangesScreen(changes, confirmDiscard, actions)
            }
        }
    }

    @Test
    fun `the include box toggles the file, the row selects it`() {
        show(SampleChanges)
        compose.onNodeWithTag("${TAG_INCLUDE}docs/new-guide.md").performClick()
        compose.onNodeWithTag("${TAG_FILE}README.md").performClick()
        assertEquals(listOf("toggle docs/new-guide.md", "select README.md"), sent)
    }

    @Test
    fun `active filters narrow the list and chips toggle them`() {
        show(SampleChanges.copy(filterNew = true))
        compose.onNodeWithText("1 of 5 changed files").assertExists()
        compose.onNodeWithTag("${TAG_FILE}README.md").assertDoesNotExist()
        compose.onNodeWithTag("${TAG_FILTER}deleted").performScrollTo().performClick()
        assertEquals(listOf("filter deleted"), sent)
    }

    @Test
    fun `stash banner restores and conflicts show a badge`() {
        show(SampleChanges.copy(conflicts = 2u))
        compose.onNodeWithTag(TAG_CONFLICTS).assertExists()
        compose.onNodeWithText("Restore").performClick()
        assertEquals(listOf("restore"), sent)
    }

    @Test
    fun `no files shows the blank slate`() {
        show(EmptyChanges)
        compose.onNodeWithTag(TAG_EMPTY).assertExists()
        compose.onNodeWithText("No local changes").assertExists()
    }

    @Test
    fun `the commit bar needs an included file`() {
        show(SampleChanges.copy(includedCount = 0u))
        compose.onNodeWithTag(TAG_COMMIT_BAR).assertIsNotEnabled()
    }

    @Test
    fun `undo bar after a commit`() {
        show(SampleChanges.copy(form = SampleForm.copy(lastCommitSha = "abc", lastCommitSummary = "Test commit")))
        compose.onNodeWithText("Test commit").assertExists()
        compose.onNodeWithTag(TAG_UNDO).performClick()
        assertEquals(listOf("undo"), sent)
    }
}

/** The commit form's Commit button: a summary and an included file, and no commit running. */
@RunWith(AndroidJUnit4::class)
class CommitFormTest {

    @get:Rule
    val compose = createComposeRule()

    private var committed = 0

    private fun show(included: Int, summary: String, committing: Boolean = false) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                var text by remember { mutableStateOf(summary) }
                CommitForm(
                    form = SampleForm.copy(committing = committing),
                    includedCount = included,
                    summary = text,
                    description = "",
                    onSummary = { text = it },
                    onDescription = {},
                    onCommit = { committed++ },
                )
            }
        }
    }

    @Test
    fun `blank summary disables commit until something is typed`() {
        show(included = 2, summary = "")
        compose.onNodeWithTag(TAG_COMMIT).assertIsNotEnabled()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_SUMMARY))).performTextInput("Fix it")
        compose.onNodeWithTag(TAG_COMMIT).assertIsEnabled().performClick()
        assertEquals(1, committed)
    }

    @Test
    fun `nothing included disables commit`() {
        show(included = 0, summary = "Fix it")
        compose.onNodeWithTag(TAG_COMMIT).assertIsNotEnabled()
    }

    @Test
    fun `a running commit disables commit`() {
        show(included = 1, summary = "Fix it", committing = true)
        compose.onNodeWithTag(TAG_COMMIT).assertIsNotEnabled()
    }
}

/** A short window (phone landscape): the chips hide behind the filter button, the Undo bar gives way to the rows. */
@RunWith(AndroidJUnit4::class)
@Config(qualifiers = "w800dp-h360dp")
class ChangesShortHeightTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `chips behind the filter button and no undo bar while there are files`() {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                ChangesScreen(
                    SampleChanges.copy(form = SampleForm.copy(lastCommitSha = "abc", lastCommitSummary = "Test commit")),
                    true,
                    NoChangesActions,
                )
            }
        }
        compose.onNodeWithTag("${TAG_FILTER}new").assertDoesNotExist()
        compose.onNodeWithTag(TAG_UNDO).assertDoesNotExist()
        compose.onNodeWithTag(TAG_FILTER_TOGGLE).performClick()
        compose.onNodeWithTag("${TAG_FILTER}new").assertExists()
    }
}
