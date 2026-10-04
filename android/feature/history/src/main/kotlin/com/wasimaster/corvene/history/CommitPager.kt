package com.wasimaster.corvene.history

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.setValue
import com.wasimaster.corvene.ffi.gen.CommitVm
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * The commits of the History list, read from the engine in windows of
 * [pageSize] (`history(repo, start, count)`) around what is on screen and
 * kept in an LRU of [maxPages] pages. [total] follows the engine's
 * `totalLoaded` ([resize]); [refresh] re-reads the pages in memory after a
 * state change (a selection flips `selected`) and replaces only the pages
 * that differ.
 */
@Stable
class CommitPager(
    total: Int,
    private val pageSize: Int = PAGE_SIZE,
    private val maxPages: Int = MAX_PAGES,
    private val load: suspend (start: Int, count: Int) -> List<CommitVm>,
) {
    private val pages = mutableStateMapOf<Int, List<CommitVm>>()
    private val recency = LinkedHashSet<Int>()
    private val mutex = Mutex()

    /** How many commits the list shows. */
    var total by mutableIntStateOf(total)
        private set

    /** The page numbers in memory (tests). */
    val loadedPages: Set<Int> get() = pages.keys.toSet()

    /** The commit at [index], or null while its page loads. */
    fun commit(index: Int): CommitVm? = pages[index / pageSize]?.getOrNull(index % pageSize)

    /** The engine loaded more (or fewer, after a compare): the last, partial page is read again. */
    fun resize(newTotal: Int) {
        if (newTotal == total) return
        // the old last page may be partial, and pages past the new end are gone
        val keep = minOf(total, newTotal) / pageSize
        pages.keys.filter { it >= keep }.forEach { pages.remove(it) }
        recency.retainAll(pages.keys)
        total = newTotal
    }

    /** Loads the pages holding [first]..[last] and one either side. */
    suspend fun ensure(first: Int, last: Int) = mutex.withLock {
        if (total == 0) return@withLock
        val lastPage = (total - 1) / pageSize
        val from = (first / pageSize - 1).coerceAtLeast(0)
        val to = (last / pageSize + 1).coerceAtMost(lastPage)
        for (page in from..to) {
            recency.remove(page)
            recency.add(page)
            if (page !in pages) {
                val rows = load(page * pageSize, pageSize)
                if (rows.isNotEmpty()) pages[page] = rows
            }
        }
        while (recency.size > maxPages) {
            val oldest = recency.first()
            recency.remove(oldest)
            pages.remove(oldest)
        }
    }

    /** Re-reads every page in memory; unchanged pages are left alone (no recomposition). */
    suspend fun refresh() = mutex.withLock {
        for (page in pages.keys.toList()) {
            val rows = load(page * pageSize, pageSize)
            when {
                rows.isEmpty() -> pages.remove(page)
                rows != pages[page] -> pages[page] = rows
            }
        }
    }

    /** The SHAs of the commits from [from] to [to] (inclusive, either order) that are loaded. */
    fun shas(from: Int, to: Int): List<String> = (minOf(from, to)..maxOf(from, to)).mapNotNull { commit(it)?.sha }

    companion object {
        const val PAGE_SIZE = 100
        const val MAX_PAGES = 10
    }
}
