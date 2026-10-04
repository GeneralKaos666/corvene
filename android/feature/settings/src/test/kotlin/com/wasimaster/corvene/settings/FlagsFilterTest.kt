package com.wasimaster.corvene.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The desktop dialog's row rule: bug fixes, All / On / Off, the search, the category order. */
class FlagsFilterTest {

    private fun slugs(query: String = "", filter: FlagFilter = FlagFilter.All, bugFixes: Boolean = false) =
        SampleFlags.visibleGroups(query, filter, bugFixes).flatMap { group -> group.flags.map { it.slug } }

    @Test
    fun `groups follow the engine's category order`() {
        assertEquals(listOf("Changes", "History", "Appearance"), SampleFlags.visibleGroups("", FlagFilter.All, false).map { it.category })
    }

    @Test
    fun `bug fixes only when shown`() {
        assertTrue("diff-check-marks-fix" !in slugs())
        assertTrue("diff-check-marks-fix" in slugs(bugFixes = true))
    }

    @Test
    fun `on means a deviation from GitHub Desktop`() {
        assertEquals(listOf("renamed-files-filter", "history-page-size"), slugs(filter = FlagFilter.On))
        assertTrue("renamed-files-filter" !in slugs(filter = FlagFilter.Off))
    }

    @Test
    fun `the search reads title, summary and ident in any case`() {
        assertEquals(listOf("history-page-size"), slugs("PAGE SIZE"))
        assertEquals(listOf("design-style"), slugs("112-design"))
    }

    @Test
    fun `restart and overrides are read off the flags`() {
        assertTrue(SampleFlags.anyOverridden)
        assertTrue(!SampleFlags.restartPending)
        assertEquals("Corvene", SampleFlags.presetTitle)
    }
}
