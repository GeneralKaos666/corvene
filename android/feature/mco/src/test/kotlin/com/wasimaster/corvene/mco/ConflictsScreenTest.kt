package com.wasimaster.corvene.mco

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.ConflictsVm
import com.wasimaster.corvene.ffi.gen.ResolutionVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The conflicts screen: per-file ours / theirs, Continue only when resolved, Abort and close. */
@RunWith(AndroidJUnit4::class)
class ConflictsScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val sent = mutableListOf<String>()

    private val actions = object : ConflictsActions {
        override fun resolve(path: String, resolution: ResolutionVm?) {
            sent += "resolve $path $resolution"
        }

        override fun abort() {
            sent += "abort"
        }

        override fun continueOperation() {
            sent += "continue"
        }

        override fun close() {
            sent += "close"
        }
    }

    private fun show(conflicts: ConflictsVm) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                ConflictsScreen(conflicts, actions)
            }
        }
    }

    @Test
    fun `ours resolves a file, the chosen side again unresolves it`() {
        show(SampleConflicts)
        compose.onNode(hasText("Use ours") and hasAnyAncestor(hasTestTag("${TAG_RESOLVE}src/main.rs"))).performClick()
        compose.onNode(hasText("Use ours") and hasAnyAncestor(hasTestTag("${TAG_RESOLVE}README.md"))).performClick()
        compose.onNode(hasText("Use theirs") and hasAnyAncestor(hasTestTag("${TAG_RESOLVE}README.md"))).performClick()
        assertEquals(listOf("resolve src/main.rs OURS", "resolve README.md null", "resolve README.md THEIRS"), sent)
    }

    @Test
    fun `continue waits for every file`() {
        show(SampleConflicts)
        compose.onNodeWithText("1 conflicted file").assertExists()
        compose.onNodeWithTag(TAG_CONTINUE).assertIsNotEnabled()
        compose.onNodeWithTag(TAG_ABORT).performClick()
        assertEquals(listOf("abort"), sent)
    }

    @Test
    fun `resolved continues and back closes`() {
        show(
            SampleConflicts.copy(
                files = SampleConflicts.files.map { it.copy(resolution = ResolutionVm.THEIRS, unresolved = false) },
                unresolvedCount = 0u,
            ),
        )
        compose.onNodeWithText("All conflicts resolved").assertExists()
        compose.onNodeWithTag(TAG_CONTINUE).assertIsEnabled().performClick()
        compose.onNodeWithText("Continue rebase").assertExists()
        compose.onNodeWithTag(TAG_CONFLICTS).assertExists()
        assertEquals(listOf("continue"), sent)
    }
}
