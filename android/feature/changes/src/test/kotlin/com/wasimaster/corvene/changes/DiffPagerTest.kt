package com.wasimaster.corvene.changes

import com.wasimaster.corvene.ffi.gen.DiffRowKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowVm
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** The pager reads only the pages around the visible rows and keeps a bounded LRU of them. */
class DiffPagerTest {

    private val requests = mutableListOf<Pair<Int, Int>>()
    private var selected = true

    private fun rows(count: Int): (Int, Int) -> List<DiffRowVm> = { start, size ->
        requests += start to size
        (start until minOf(start + size, count)).map {
            DiffRowVm(it.toUInt(), DiffRowKindVm.ADD, "line $it", null, it.toUInt(), selected, false, emptyList())
        }
    }

    private fun pager(count: Int, maxPages: Int = 4): DiffPager {
        val load = rows(count)
        return DiffPager(generation = 1, rowCount = count, pageSize = 100, maxPages = maxPages) { start, size -> load(start, size) }
    }

    @Test
    fun `loads the visible page first, then one page either side`() = runTest {
        val pager = pager(10_000)
        pager.ensure(first = 1_050, last = 1_080)
        assertEquals(listOf(1_000 to 100, 900 to 100, 1_100 to 100), requests)
        assertEquals(setOf(9, 10, 11), pager.loadedPages)
        assertEquals("line 1050", pager.row(1_050)?.text)
        assertNull(pager.row(5_000))
    }

    @Test
    fun `scrolling far away evicts the least recently used pages`() = runTest {
        val pager = pager(10_000, maxPages = 4)
        pager.ensure(0, 30)
        pager.ensure(5_000, 5_030)
        assertEquals(4, pager.loadedPages.size)
        assertTrue(50 in pager.loadedPages)
        assertNotNull(pager.row(5_000))
        assertNull(pager.row(0))
    }

    @Test
    fun `pages already loaded are not fetched again`() = runTest {
        val pager = pager(1_000)
        pager.ensure(0, 10)
        val before = requests.size
        pager.ensure(20, 40)
        assertEquals(before, requests.size)
    }

    @Test
    fun `the last page stops at the row count`() = runTest {
        val pager = pager(250)
        pager.ensure(240, 249)
        assertEquals(setOf(1, 2), pager.loadedPages)
        assertEquals("line 249", pager.row(249)?.text)
        assertNull(pager.row(250))
    }

    @Test
    fun `refresh replaces only pages that changed`() = runTest {
        val pager = pager(150)
        pager.ensure(0, 10)
        val first = pager.row(5)
        pager.refresh()
        assertSame(first, pager.row(5))
        selected = false
        pager.refresh()
        assertEquals(false, pager.row(5)?.selected)
    }

    @Test
    fun `a stale generation answers nothing and leaves the pager empty`() = runTest {
        val pager = DiffPager(generation = 1, rowCount = 500, pageSize = 100) { _, _ -> emptyList() }
        pager.ensure(0, 10)
        assertTrue(pager.loadedPages.isEmpty())
    }

    @Test
    fun `the widest row sets the horizontal extent`() = runTest {
        val pager = pager(120)
        pager.ensure(0, 10)
        assertEquals("line 119".length, pager.maxColumns)
    }
}
