package com.wasimaster.corvene.changes

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.DiffHeaderVm
import com.wasimaster.corvene.ffi.gen.IncludeVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import kotlinx.coroutines.flow.drop

/**
 * The Changes tab wired to the engine: `changes(repo)` after every state
 * change, the settings for the discard prompt; every action is a dispatch.
 * [onOpenDiff] runs after a file is selected (compact widths push the diff).
 */
@Composable
fun ChangesRoute(repo: Long, onOpenDiff: () -> Unit, modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val id = repo.toULong()
    val query by rememberCoreQuery(repo) { changes(id) }
    val settings by rememberCoreQuery { settings() }
    val openDiff by rememberUpdatedState(onOpenDiff)
    val actions = remember(core, id) {
        object : ChangesActions {
            override fun select(path: String) {
                core.dispatch { selectFile(id, path) }
                openDiff()
            }

            override fun toggleIncluded(path: String) = core.dispatch { toggleFileIncluded(id, path) }

            override fun discard(paths: List<String>) = core.dispatch { discardChanges(id, paths) }

            override fun toggleFilter(option: FilterOption) = core.dispatch { toggleFilterOption(id, option.key) }

            override fun clearFilters() = core.dispatch { clearFilterOptions(id) }

            override fun commit(summary: String, description: String) = core.dispatch { commit(id, summary, description) }

            override fun undo() = core.dispatch { undoCommit(id) }

            override fun restoreStash() = core.dispatch { popStash(id) }
        }
    }
    val changes = query.value
    when {
        changes != null -> ChangesScreen(changes, settings.value?.confirmDiscardChanges ?: true, actions, modifier)
        query.error != null -> Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(
                stringResource(R.string.chg_failed, query.error.orEmpty()),
                Modifier.padding(24.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = CorveneTheme.colors.danger.fg,
            )
        }
        else -> Box(modifier.fillMaxSize()) { ProgressBar(null, Modifier.align(Alignment.TopCenter)) }
    }
}

/**
 * The selected file's diff wired to the engine: the header after every state
 * change, a [DiffPager] per (generation, row count) reading `diffRows`, and a
 * refresh of the visible pages after other changes (a toggled line keeps the
 * generation). [contentPadding] keeps the last rows above the system bars.
 */
@Composable
fun DiffRoute(repo: Long, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val id = repo.toULong()
    val header by rememberCoreQuery(repo) { diffHeader(id) }
    val settings by rememberCoreQuery { settings() }
    val value = header.value
    val generation = value?.generation
    val rows = value?.rowCount?.toInt() ?: 0
    val pager = remember(repo, generation, rows) {
        generation?.let { gen ->
            DiffPager(gen.toLong(), rows) { start, count -> core.query { diffRows(id, gen, start.toUInt(), count.toUInt()) } }
        }
    }
    val cache = remember { DiffLineCache() }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(pager, lifecycle) {
        val current = pager ?: return@LaunchedEffect
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            core.version.drop(1).collect { current.refresh() }
        }
    }
    val actions = remember(core, id, value?.path) {
        val path = value?.path
        object : DiffActions {
            override fun toggleFileIncluded() {
                if (path != null) core.dispatch { toggleFileIncluded(id, path) }
            }

            override fun toggleLine(index: Int) {
                if (path != null) core.dispatch { toggleDiffLine(id, path, index.toUInt()) }
            }

            override fun setHideWhitespace(hide: Boolean) = core.dispatch { setHideWhitespaceInDiff(false, hide) }
        }
    }
    if (value == null) {
        Box(modifier.fillMaxSize()) { ProgressBar(null, Modifier.align(Alignment.TopCenter)) }
    } else {
        DiffScreen(value, pager, cache, settings.value?.hideWhitespaceInChangesDiff ?: false, actions, modifier, contentPadding)
    }
}

/**
 * The selected commit's selected file (History) wired to the engine: the
 * commit detail for its generation and counts, rows from `commitDiffRows`,
 * read-only (no include box, no line toggles). [contentPadding] as for
 * [DiffRoute].
 */
@Composable
fun CommitDiffRoute(repo: Long, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val id = repo.toULong()
    val detail by rememberCoreQuery(repo) { commitDetail(id) }
    val settings by rememberCoreQuery { settings() }
    val value = detail.value
    val header = value?.let {
        DiffHeaderVm(
            repo = id,
            path = it.selectedFile,
            kind = it.diffKind,
            generation = it.diffGeneration,
            rowCount = it.diffRowCount,
            hunkCount = 0u,
            linesAdded = it.diffLinesAdded,
            linesDeleted = it.diffLinesDeleted,
            include = IncludeVm.NONE,
        )
    }
    val generation = header?.generation
    val rows = header?.rowCount?.toInt() ?: 0
    val pager = remember(repo, generation, rows) {
        generation?.let { gen ->
            DiffPager(gen.toLong(), rows) { start, count -> core.query { commitDiffRows(id, gen, start.toUInt(), count.toUInt()) } }
        }
    }
    val cache = remember { DiffLineCache() }
    val actions = remember(core) {
        object : DiffActions {
            override fun toggleFileIncluded() = Unit

            override fun toggleLine(index: Int) = Unit

            override fun setHideWhitespace(hide: Boolean) = core.dispatch { setHideWhitespaceInDiff(true, hide) }
        }
    }
    if (header == null) {
        Box(modifier.fillMaxSize()) { ProgressBar(null, Modifier.align(Alignment.TopCenter)) }
    } else {
        DiffScreen(
            header,
            pager,
            cache,
            settings.value?.hideWhitespaceInHistoryDiff ?: false,
            actions,
            modifier,
            contentPadding,
            readOnly = true,
        )
    }
}
