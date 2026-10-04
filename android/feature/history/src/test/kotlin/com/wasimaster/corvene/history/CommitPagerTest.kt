package com.wasimaster.corvene.history

import com.wasimaster.corvene.ffi.gen.CommitVm
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CommitPagerTest {

    private val loads = mutableListOf<Pair<Int, Int>>()
    private var commits = (0 until 450).map { CommitVm("s$it", "c$it", "a", "e", 0, emptyList(), false, false) }

    private fun pager() = CommitPager(commits.size, pageSize = 100, maxPages = 3) { start, count ->
        loads += start to count
        commits.drop(start).take(count)
    }

    @Test
    fun `windows of a hundred around what is visible`() = runTest {
        val pager = pager()
        pager.ensure(150, 160)
        assertEquals(listOf(0 to 100, 100 to 100, 200 to 100), loads)
        assertEquals("s155", pager.commit(155)?.sha)
        assertNull(pager.commit(399))
    }

    @Test
    fun `old pages fall out of the cache`() = runTest {
        val pager = pager()
        pager.ensure(0, 10)
        pager.ensure(420, 440)
        assertEquals(setOf(3, 4), pager.loadedPages.filter { it >= 3 }.toSet())
        assertEquals(3, pager.loadedPages.size)
    }

    @Test
    fun `resize reads the partial last page again, refresh replaces changed pages`() = runTest {
        val pager = pager()
        pager.ensure(440, 449)
        commits = commits + (450 until 520).map { CommitVm("s$it", "c$it", "a", "e", 0, emptyList(), false, false) }
        pager.resize(520)
        assertNull(pager.commit(460))
        pager.ensure(460, 470)
        assertEquals("s460", pager.commit(460)?.sha)
        commits = commits.map { if (it.sha == "s460") it.copy(selected = true) else it }
        pager.refresh()
        assertEquals(true, pager.commit(460)?.selected)
        assertEquals(listOf("s459", "s460", "s461"), pager.shas(461, 459))
    }
}
