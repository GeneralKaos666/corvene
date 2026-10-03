package com.wasimaster.corvene.repositories

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.CounterLabel
import com.wasimaster.corvene.design.DesignStyleSamples
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm

/**
 * The repository list: [list] grouped like GHD's Repository foldout, the
 * selected one highlighted, indicators at the end of each row (a dot for
 * uncommitted changes, ahead/behind counts). Tap selects, long-press offers
 * removal, pull refreshes the indicators. Empty: a Blankslate to add one.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RepositoryListScreen(
    list: RepoListVm,
    refreshing: Boolean,
    onSelect: (RepoVm) -> Unit,
    onRemove: (RepoVm) -> Unit,
    onAdd: () -> Unit,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    var removing by rememberSaveable { mutableStateOf<Long?>(null) }
    val groups = remember(list) { groupRepositories(list) }
    PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = onRefresh,
        modifier = modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas),
    ) {
        if (list.repositories.isEmpty()) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
                item {
                    Blankslate(
                        icon = Octicons.Repo,
                        title = stringResource(R.string.repo_empty_title),
                        description = stringResource(R.string.repo_empty_description),
                        primaryAction = {
                            PrimerButton(
                                stringResource(R.string.repo_add),
                                onAdd,
                                variant = PrimerButtonVariant.Primary,
                                leadingIcon = Octicons.Plus,
                                modifier = Modifier.testTag(TAG_ADD),
                            )
                        },
                    )
                }
            }
        } else {
            LazyColumn(Modifier.fillMaxSize().testTag(TAG_LIST), contentPadding = contentPadding) {
                groups.forEach { group ->
                    item(key = "header:${group.kind}:${group.title}", contentType = "header") {
                        ActionListGroupHeader(
                            when (group.kind) {
                                RepositoryGroup.Kind.Recent -> stringResource(R.string.repo_group_recent)
                                RepositoryGroup.Kind.Owner -> group.title
                                RepositoryGroup.Kind.Other -> stringResource(R.string.repo_group_other)
                            },
                        )
                    }
                    items(group.repositories, key = { "${group.kind}:${group.title}:${it.id}" }, contentType = { "row" }) { repo ->
                        RepositoryRow(
                            repo,
                            selected = repo.id == list.selected,
                            onClick = { onSelect(repo) },
                            onLongClick = { removing = repo.id.toLong() },
                        )
                        ActionListDivider()
                    }
                }
                item(key = "add", contentType = "row") {
                    ActionListItem(
                        stringResource(R.string.repo_add),
                        onClick = onAdd,
                        leading = { IconTile(Octicons.Plus, CorveneTheme.colors.success) },
                        modifier = Modifier.testTag(TAG_ADD),
                    )
                }
            }
        }
    }
    val target = removing?.let { id -> list.repositories.firstOrNull { it.id.toLong() == id } }
    if (target != null) {
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text(stringResource(R.string.repo_remove_title, target.name)) },
            text = { Text(stringResource(R.string.repo_remove_message)) },
            confirmButton = {
                PrimerButton(
                    stringResource(R.string.repo_remove),
                    {
                        removing = null
                        onRemove(target)
                    },
                    variant = PrimerButtonVariant.Danger,
                )
            },
            dismissButton = {
                PrimerButton(stringResource(R.string.repo_cancel), { removing = null }, variant = PrimerButtonVariant.Invisible)
            },
        )
    }
}

@Composable
private fun RepositoryRow(repo: RepoVm, selected: Boolean, onClick: () -> Unit, onLongClick: () -> Unit) {
    val colors = CorveneTheme.colors
    ActionListItem(
        title = repo.name,
        description = if (repo.missing) stringResource(R.string.repo_missing) else repo.branch ?: repo.path,
        selected = selected,
        onClick = onClick,
        onLongClick = onLongClick,
        modifier = Modifier.testTag("$TAG_ROW${repo.id}"),
        leading = {
            when {
                repo.missing -> IconTile(Octicons.Alert, colors.attention)
                repo.github != null -> IconTile(Octicons.Repo, colors.accent)
                else -> IconTile(Octicons.DeviceDesktop, colors.done)
            }
        },
        trailing = {
            val ahead = repo.ahead?.toInt() ?: 0
            val behind = repo.behind?.toInt() ?: 0
            val aheadLabel = stringResource(R.string.repo_ahead, ahead)
            val behindLabel = stringResource(R.string.repo_behind, behind)
            if (ahead > 0) CounterLabel("↑$ahead", Modifier.semantics { contentDescription = aheadLabel })
            if (behind > 0) CounterLabel("↓$behind", Modifier.semantics { contentDescription = behindLabel })
            if (repo.changedFiles > 0u) {
                val label = stringResource(R.string.repo_changed_files)
                Box(
                    Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(colors.accent.fg)
                        .semantics { contentDescription = label },
                )
            }
        },
    )
}

const val TAG_LIST = "repo_list"
const val TAG_ADD = "repo_add"
const val TAG_ROW = "repo_row_"

internal val SampleList = RepoListVm(
    selected = 2u,
    recent = listOf(2u, 3u),
    repositories = listOf(
        RepoVm(1u, "corvene", "/data/user/0/com.wasimaster.corvene/files/repositories/corvene",
            "wasi-master/corvene", false, "main", 0u, 0u, 0u),
        RepoVm(2u, "desktop", "/storage/emulated/0/Code/desktop", "desktop/desktop", false, "development", 3u, 2u, 1u),
        RepoVm(3u, "notes", "/storage/emulated/0/Documents/notes", null, false, "main", 0u, null, null),
        RepoVm(4u, "old-project", "/storage/emulated/0/old-project", null, true, null, 0u, null, null),
    ),
    signedIn = false,
    welcomeCompleted = true,
)

internal val EmptyList = RepoListVm(null, emptyList(), emptyList(), signedIn = false, welcomeCompleted = false)

@Preview(widthDp = 360, heightDp = 1500)
@Composable
private fun RepositoryListScreenPreview() {
    DesignStyleSamples {
        Box(Modifier.size(360.dp, 440.dp)) {
            RepositoryListScreen(SampleList, refreshing = false, onSelect = {}, onRemove = {}, onAdd = {}, onRefresh = {})
        }
    }
}

@Preview(widthDp = 360, heightDp = 1100)
@Composable
private fun RepositoryListEmptyPreview() {
    DesignStyleSamples {
        Box(Modifier.size(360.dp, 320.dp)) {
            RepositoryListScreen(EmptyList, refreshing = false, onSelect = {}, onRemove = {}, onAdd = {}, onRefresh = {})
        }
    }
}
