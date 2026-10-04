package com.wasimaster.corvene.changes

import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.DiffHeaderVm
import com.wasimaster.corvene.ffi.gen.DiffKindVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The diff pages its rows in, taps toggle lines, the header box toggles the file. */
@RunWith(AndroidJUnit4::class)
class DiffScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val sent = mutableListOf<String>()

    private val actions = object : DiffActions {
        override fun toggleFileIncluded() {
            sent += "file"
        }

        override fun toggleLine(index: Int) {
            sent += "line $index"
        }

        override fun setHideWhitespace(hide: Boolean) {
            sent += "whitespace $hide"
        }
    }

    private fun show(header: DiffHeaderVm = SampleHeader) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                DiffScreen(header, remember { samplePager() }, remember { DiffLineCache() }, false, actions)
            }
        }
    }

    @Test
    fun `tapping an added line toggles it, context lines do nothing`() {
        show()
        compose.onNodeWithTag("${TAG_DIFF_ROW}3").performClick()
        compose.onNodeWithTag("${TAG_DIFF_ROW}1").performClick()
        compose.onNodeWithTag(TAG_DIFF_INCLUDE).performClick()
        assertEquals(listOf("line 3", "file"), sent)
    }

    @Test
    fun `binary files show a notice instead of rows`() {
        show(SampleHeader.copy(kind = DiffKindVm.BINARY))
        compose.onNodeWithText("This binary file has changed.").assertExists()
        compose.onNodeWithTag(TAG_DIFF_LIST).assertDoesNotExist()
    }

    @Test
    fun `a large diff waits for Show diff`() {
        show(SampleHeader.copy(kind = DiffKindVm.LARGE_TEXT))
        compose.onNodeWithTag(TAG_DIFF_LIST).assertDoesNotExist()
        compose.onNodeWithText("Show diff").performClick()
        compose.onNodeWithTag(TAG_DIFF_LIST).assertExists()
    }
}
