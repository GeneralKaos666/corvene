package com.wasimaster.corvene.history

import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performTouchInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.CommitVm
import com.wasimaster.corvene.ffi.gen.HistoryVm
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The History list against fixed view models; the actions record what it sends. */
@RunWith(AndroidJUnit4::class)
class HistoryScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val sent = mutableListOf<String>()

    private val actions = object : HistoryActions by NoHistoryActions {
        override fun select(sha: String) {
            sent += "select $sha"
        }

        override fun selectRange(shas: List<String>) {
            sent += "range $shas"
        }

        override fun loadMore() {
            sent += "more"
        }

        override fun compare(branch: String?, mode: String) {
            sent += "compare $branch $mode"
        }

        override fun merge(branch: String) {
            sent += "merge $branch"
        }

        override fun revert(sha: String) {
            sent += "revert $sha"
        }

        override fun copySha(sha: String) {
            sent += "copy $sha"
        }
    }

    private fun commits(n: Int) = (0 until n).map { i ->
        CommitVm("sha$i", "Commit $i", "Wasi", "w@example.com", 1_791_000_000L - i * 60, emptyList(), false, false)
    }

    private fun show(history: HistoryVm, pager: CommitPager, comparison: Comparison? = null) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                HistoryScreen(
                    history,
                    remember { pager },
                    comparison,
                    listOf("main", "feature-a"),
                    "main",
                    false,
                    actions,
                    now = SAMPLE_NOW,
                )
            }
        }
    }

    @Test
    fun `scrolling near the end asks for more, once per count`() {
        val all = commits(150)
        show(SampleHistory.copy(totalLoaded = 150u, exhausted = false), samplePager(all))
        compose.waitForIdle()
        assertTrue("no load before the end: $sent", "more" !in sent)
        compose.onNodeWithTag(TAG_LIST).performScrollToIndex(140)
        compose.waitUntil(WAIT_MS) { "more" in sent }
        compose.onNodeWithTag(TAG_LIST).performScrollToIndex(145)
        compose.waitUntil(WAIT_MS) { compose.onAllNodesWithText("Commit 145").fetchSemanticsNodes().isNotEmpty() }
        assertEquals(1, sent.count { it == "more" })
    }

    @Test
    fun `an exhausted history never asks for more`() {
        show(SampleHistory.copy(totalLoaded = 30u, exhausted = true), samplePager(commits(30)))
        compose.onNodeWithTag(TAG_LIST).performScrollToIndex(29)
        compose.waitForIdle()
        assertTrue("more" !in sent)
    }

    @Test
    fun `a tap selects, select multiple then a tap picks a contiguous range`() {
        show(SampleHistory, samplePager())
        compose.onNodeWithTag("${TAG_COMMIT}c3d4e5f6a7").performClick()
        compose.onNodeWithTag("${TAG_COMMIT}b2c3d4e5f6").performTouchInput { longClick() }
        compose.onNodeWithText("Select multiple").performClick()
        compose.onNodeWithTag(TAG_SELECTION).assertExists()
        compose.onNodeWithTag("${TAG_COMMIT}d4e5f6a7b8").performClick()
        assertEquals(listOf("select c3d4e5f6a7", "range [b2c3d4e5f6, c3d4e5f6a7, d4e5f6a7b8]"), sent)
    }

    @Test
    fun `the commit menu reverts and copies`() {
        show(SampleHistory, samplePager())
        compose.onNodeWithTag("${TAG_COMMIT}d4e5f6a7b8").performTouchInput { longClick() }
        compose.onNodeWithTag(TAG_MENU_REVERT).performClick()
        compose.onNodeWithTag("${TAG_COMMIT}d4e5f6a7b8").performTouchInput { longClick() }
        compose.onNodeWithTag(TAG_MENU_COPY).performClick()
        assertEquals(listOf("revert d4e5f6a7b8", "copy d4e5f6a7b8"), sent)
    }

    @Test
    fun `compare picks a branch, switches to ahead, merges and ends`() {
        show(SampleHistory, samplePager())
        compose.onNodeWithTag(TAG_COMPARE).performClick()
        compose.onNodeWithTag("${TAG_COMPARE_BRANCH}feature-a").performClick()
        assertEquals(listOf("compare feature-a behind"), sent)
    }

    @Test
    fun `while comparing behind, merge merges the branch in`() {
        show(SampleHistory, samplePager(), Comparison("feature-a", ahead = false))
        compose.onNodeWithTag(TAG_MERGE).performClick()
        compose.onNodeWithText("Ahead").performClick()
        compose.onNodeWithTag(TAG_COMPARE_END).performClick()
        assertEquals(listOf("merge feature-a", "compare feature-a ahead", "compare null behind"), sent)
    }
}

private const val WAIT_MS = 5_000L

/** The commit detail: files select, the SHA copies, the body expands. */
@RunWith(AndroidJUnit4::class)
class CommitDetailScreenTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `files select and the sha copies`() {
        val sent = mutableListOf<String>()
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubDesktop, ColorMode.Light, highContrast = false, dynamicColor = false) {
                CommitDetailScreen(
                    SampleDetail,
                    object : CommitDetailActions {
                        override fun selectFile(path: String) {
                            sent += "file $path"
                        }

                        override fun copySha(sha: String) {
                            sent += "copy $sha"
                        }
                    },
                    now = SAMPLE_NOW,
                )
            }
        }
        compose.onNodeWithText("4 changed files").assertExists()
        compose.onNodeWithText("+412").assertExists()
        compose.onNodeWithTag(TAG_EXPAND).performClick()
        compose.onNodeWithText("Show less").assertExists()
        compose.onNodeWithTag("${TAG_FILE}old/Picker.kt").performClick()
        compose.onNodeWithTag(TAG_COPY).performClick()
        assertEquals(listOf("file old/Picker.kt", "copy b2c3d4e5f6"), sent)
    }
}
