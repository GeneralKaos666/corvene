package com.wasimaster.corvene.repositories

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.longClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The screen against fixed view models: no engine, the lambdas record what it sends. */
@RunWith(AndroidJUnit4::class)
class RepositoryListScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val selected = mutableListOf<RepoVm>()
    private val removed = mutableListOf<RepoVm>()
    private var added = 0

    private fun show(list: RepoListVm) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                RepositoryListScreen(
                    list = list,
                    refreshing = false,
                    onSelect = { selected += it },
                    onRemove = { removed += it },
                    onAdd = { added++ },
                    onRefresh = {},
                )
            }
        }
    }

    @Test
    fun `empty list offers to add a repository`() {
        show(EmptyList)
        compose.onNodeWithText("No repositories yet").assertIsDisplayed()
        compose.onNodeWithTag(TAG_ADD).performClick()
        assertEquals(1, added)
    }

    @Test
    fun `rows are grouped and tapping selects`() {
        show(SampleList)
        compose.onNodeWithText("Recent").assertIsDisplayed()
        compose.onNodeWithText("Other").assertIsDisplayed()
        // notes is in Recent and in Other; either row selects it
        compose.onAllNodesWithTag("${TAG_ROW}3").onFirst().performClick()
        assertEquals(listOf("notes"), selected.map { it.name })
    }

    @Test
    fun `long press asks before removing`() {
        show(SampleList)
        compose.onNodeWithTag(TAG_LIST).performScrollToNode(hasTestTag("${TAG_ROW}4"))
        compose.onNodeWithTag("${TAG_ROW}4").performTouchInput { longClick() }
        compose.onNodeWithText("Remove old-project?").assertIsDisplayed()
        assertTrue(removed.isEmpty())
        compose.onNodeWithText("Remove").performClick()
        assertEquals(listOf("old-project"), removed.map { it.name })
    }
}
