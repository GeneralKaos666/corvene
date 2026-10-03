package com.wasimaster.corvene.repositories

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.RepoVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.rememberFolderPicker
import kotlinx.coroutines.delay

/**
 * The repository list wired to the engine: queries `repoList` after every
 * state change and sends select / remove / add / refresh. [onOpen] runs after
 * a selection, with the repository's id.
 */
@Composable
fun RepositoryListRoute(onOpen: (Long) -> Unit, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val query by rememberCoreQuery { repoList() }
    val failure by core.startupFailure.collectAsStateWithLifecycle()
    var refreshing by remember { mutableStateOf(false) }
    val picker = rememberFolderPicker { path ->
        if (path != null) core.dispatch { addRepository(path) }
    }
    // the indicators come back as state changes; the spinner is only a receipt
    LaunchedEffect(refreshing, query.value) {
        if (refreshing) {
            delay(REFRESH_SPINNER_MS)
            refreshing = false
        }
    }
    val list = query.value
    when {
        list != null -> RepositoryListScreen(
            list = list,
            refreshing = refreshing,
            onSelect = { repo: RepoVm ->
                core.dispatch { selectRepository(repo.id) }
                onOpen(repo.id.toLong())
            },
            onRemove = { repo -> core.dispatch { removeRepository(repo.id) } },
            onAdd = picker::pick,
            onRefresh = {
                refreshing = true
                core.dispatch { refreshIndicators() }
            },
            modifier = modifier,
            contentPadding = contentPadding,
        )
        failure != null -> Box(modifier.fillMaxSize().padding(contentPadding), contentAlignment = Alignment.Center) {
            Text(
                stringResource(R.string.repo_failed, failure.orEmpty()),
                color = CorveneTheme.colors.danger.fg,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(24.dp),
            )
        }
        else -> Box(modifier.fillMaxSize().padding(contentPadding)) {
            LinearProgressIndicator(Modifier.align(Alignment.TopCenter))
        }
    }
}

private const val REFRESH_SPINNER_MS = 1_200L
