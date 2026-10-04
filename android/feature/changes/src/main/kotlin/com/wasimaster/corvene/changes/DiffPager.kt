package com.wasimaster.corvene.changes

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.setValue
import com.wasimaster.corvene.ffi.gen.DiffRowVm
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * The rows of one diff ([generation], [rowCount] rows), fetched from the
 * engine in pages of [pageSize] around what is on screen and kept in an LRU
 * of [maxPages] pages, so a 50 000-line diff costs a few hundred rows of
 * memory. [row] is snapshot state: an item whose page arrives recomposes.
 *
 * [load] answers `diffRows(repo, generation, start, count)`; an empty answer
 * means the generation went stale (the route makes a new pager for the new
 * one). [refresh] re-reads the pages in the last window after a state change
 * (a toggled line keeps the generation but changes `selected`); a page that
 * comes back equal is not replaced, so nothing recomposes.
 */
@Stable
class DiffPager(
    val generation: Long,
    val rowCount: Int,
    private val pageSize: Int = PAGE_SIZE,
    private val maxPages: Int = MAX_PAGES,
    private val load: suspend (start: Int, count: Int) -> List<DiffRowVm>,
) {
    private val pages = mutableStateMapOf<Int, List<DiffRowVm>>()

    /** Page numbers, least recently used first. */
    private val recency = LinkedHashSet<Int>()
    private val mutex = Mutex()
    private var window: IntRange = IntRange.EMPTY

    /** The widest row seen so far, in UTF-16 units (for the shared horizontal scroll). */
    var maxColumns by mutableIntStateOf(0)
        private set

    /** The page numbers in memory (tests). */
    val loadedPages: Set<Int> get() = pages.keys.toSet()

    /** The row at [index], or null while its page loads. */
    fun row(index: Int): DiffRowVm? = pages[index / pageSize]?.getOrNull(index % pageSize)

    /**
     * Makes sure rows [first]..[last] are loaded, plus a page before and after
     * (prefetch), visible pages first; evicts the least recently used pages
     * beyond [maxPages].
     */
    suspend fun ensure(first: Int, last: Int) {
        if (rowCount <= 0) return
        mutex.withLock {
            val lastPage = (rowCount - 1) / pageSize
            val visible = pageOf(first)..pageOf(last)
            val wanted = (visible.first - 1).coerceAtLeast(0)..(visible.last + 1).coerceAtMost(lastPage)
            window = wanted
            val order = visible.toList() + (wanted - visible.toSet())
            for (page in order) {
                touch(page)
                if (page !in pages) {
                    val rows = load(page * pageSize, pageSize)
                    if (rows.isEmpty()) return
                    put(page, rows)
                }
            }
            evict(wanted)
        }
    }

    /** Re-reads the pages of the last window; equal pages stay as they are. */
    suspend fun refresh() {
        mutex.withLock {
            for (page in window) {
                if (page !in pages) continue
                val rows = load(page * pageSize, pageSize)
                if (rows.isEmpty()) return
                if (rows != pages[page]) put(page, rows)
            }
        }
    }

    private fun pageOf(index: Int) = index.coerceIn(0, rowCount - 1) / pageSize

    private fun put(page: Int, rows: List<DiffRowVm>) {
        pages[page] = rows
        val widest = rows.maxOfOrNull { it.text.length } ?: 0
        if (widest > maxColumns) maxColumns = widest
    }

    private fun touch(page: Int) {
        recency.remove(page)
        recency.add(page)
    }

    private fun evict(keep: IntRange) {
        val iterator = recency.iterator()
        while (pages.size > maxPages && iterator.hasNext()) {
            val page = iterator.next()
            if (page in keep) continue
            iterator.remove()
            pages.remove(page)
        }
    }

    companion object {
        const val PAGE_SIZE = 200
        const val MAX_PAGES = 12
    }
}
