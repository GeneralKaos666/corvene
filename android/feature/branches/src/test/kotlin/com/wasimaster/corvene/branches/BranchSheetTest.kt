package com.wasimaster.corvene.branches

import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.BranchesVm
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The branch panel against fixed view models; the actions record what it sends. */
@RunWith(AndroidJUnit4::class)
class BranchSheetTest {

    @get:Rule
    val compose = createComposeRule()

    private val sent = mutableListOf<String>()
    private var dismissed = 0

    private val actions = object : BranchActions {
        override fun checkout(name: String) {
            sent += "checkout $name"
        }

        override fun create(name: String, startPoint: String?) {
            sent += "create $name from $startPoint"
        }

        override fun rename(old: String, new: String) {
            sent += "rename $old $new"
        }

        override fun delete(name: String, includeRemote: Boolean) {
            sent += "delete $name $includeRemote"
        }

        override fun merge(name: String) {
            sent += "merge $name"
        }

        override fun rebaseOnto(name: String) {
            sent += "rebase $name"
        }

        override fun compare(name: String) {
            sent += "compare $name"
        }

        override fun checkoutPullRequest(number: ULong) {
            sent += "pr $number"
        }
    }

    private fun show(branches: BranchesVm = SampleBranches, github: Boolean = true) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                BranchSheet(branches, if (github) SamplePulls else null, actions, { dismissed++ }, inline = true, now = SAMPLE_NOW)
            }
        }
    }

    @Test
    fun `groups show default, recent and other, remotes folded`() {
        show()
        compose.onNodeWithText("Default branch").assertExists()
        compose.onNodeWithText("Recent branches").assertExists()
        compose.onNodeWithText("Other branches").assertExists()
        compose.onNodeWithTag("${TAG_BRANCH}origin/release").assertDoesNotExist()
        compose.onNodeWithTag(TAG_REMOTE).performClick()
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasTestTag("${TAG_BRANCH}origin/release"))
        compose.onNodeWithTag("${TAG_BRANCH}origin/release").assertExists()
    }

    @Test
    fun `the filter narrows the list`() {
        show()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_SHEET))).performTextInput("feature-b")
        compose.onNodeWithTag("${TAG_BRANCH}feature-b").assertExists()
        compose.onNodeWithTag("${TAG_BRANCH}feature-a").assertDoesNotExist()
        compose.onNodeWithTag("${TAG_BRANCH}main").assertDoesNotExist()
    }

    @Test
    fun `a tap checks the branch out and closes, the current branch only closes`() {
        show()
        compose.onNodeWithTag("${TAG_BRANCH}feature-a").performClick()
        compose.onNodeWithTag("${TAG_BRANCH}main").performClick()
        assertEquals(listOf("checkout feature-a"), sent)
        assertEquals(2, dismissed)
    }

    @Test
    fun `long press opens the menu and merges into the current branch`() {
        show()
        compose.onNodeWithTag("${TAG_BRANCH}feature-b").performTouchInput { longClick() }
        compose.onNodeWithTag(TAG_MENU_MERGE).performClick()
        assertEquals(listOf("merge feature-b"), sent)
    }

    @Test
    fun `new branch takes the filter as its name and creates it`() {
        show()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_SHEET))).performTextInput("topic x")
        compose.onNodeWithTag(TAG_NEW).performClick()
        compose.onNodeWithText("Will be created as topic-x").assertExists()
        compose.onNodeWithTag(TAG_CREATE_CONFIRM).performClick()
        assertEquals(listOf("create topic-x from null"), sent)
    }

    @Test
    fun `an existing name cannot be created`() {
        show()
        compose.onNodeWithTag(TAG_NEW).performClick()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_NAME))).performTextInput("feature-a")
        compose.onNodeWithTag(TAG_CREATE_CONFIRM).assertIsNotEnabled()
    }

    @Test
    fun `the pull requests tab checks one out`() {
        show()
        compose.onNodeWithText("Pull requests").performClick()
        compose.onNodeWithTag("${TAG_PULL}12").performClick()
        assertEquals(listOf("pr 12"), sent)
        assertTrue(dismissed > 0)
    }

    @Test
    fun `no pull requests tab outside GitHub`() {
        show(github = false)
        compose.onNodeWithText("Pull requests").assertDoesNotExist()
    }
}

/** The stash prompt sends the strategy chosen. */
@RunWith(AndroidJUnit4::class)
class StashAndSwitchBranchDialogTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `leave on the current branch stashes, bring moves`() {
        val chosen = mutableListOf<String>()
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubDesktop, ColorMode.Dark, highContrast = false, dynamicColor = false) {
                StashAndSwitchBranchDialog("main", "feature-a", hasStash = true, onSwitch = { chosen += it }, onDismissRequest = {})
            }
        }
        compose.onNodeWithText("Your current stash will be overwritten by creating a new stash").assertExists()
        compose.onNodeWithTag(TAG_SWITCH_CONFIRM).performClick()
        compose.onNodeWithTag(TAG_SWITCH_MOVE).performClick()
        compose.onNodeWithTag(TAG_SWITCH_CONFIRM).performClick()
        assertEquals(listOf(STRATEGY_STASH, STRATEGY_MOVE), chosen)
    }
}
