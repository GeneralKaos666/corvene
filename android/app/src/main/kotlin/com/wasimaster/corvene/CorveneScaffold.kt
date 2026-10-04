package com.wasimaster.corvene

import android.text.format.DateUtils
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.VerticalDragHandle
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.layout.AnimatedPane
import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffoldRole
import androidx.compose.material3.adaptive.layout.PaneExpansionAnchor
import androidx.compose.material3.adaptive.layout.rememberPaneExpansionState
import androidx.compose.material3.adaptive.navigation.NavigableListDetailPaneScaffold
import androidx.compose.material3.adaptive.navigation.rememberListDetailPaneScaffoldNavigator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.changes.ChangesRoute
import com.wasimaster.corvene.changes.DiffRoute
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.RepositoryChromeLabels
import com.wasimaster.corvene.design.RepositoryTopChrome
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.design.SyncButtonModel
import com.wasimaster.corvene.design.UnderlineNav
import com.wasimaster.corvene.design.UnderlineNavItem
import com.wasimaster.corvene.design.isCompactWidth
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.SyncActionVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.rememberFolderPicker
import com.wasimaster.corvene.repositories.RepositoryPicker
import kotlinx.coroutines.launch

/**
 * A repository: the chrome of the style in effect ([RepositoryTopChrome]:
 * the repository switcher, the branch, the sync button, the Changes | History
 * tabs), then the tab. Compact widths show one pane and push the diff as its
 * own destination ([onOpenDiff]); medium and expanded widths put the changes
 * list and the diff side by side in a list-detail scaffold with a draggable
 * divider. The only place that touches adaptive navigation.
 */
@Composable
fun CorveneScaffold(
    id: Long,
    onBack: () -> Unit,
    onOpenDiff: () -> Unit,
    onOpenRepository: (Long) -> Unit,
    onAppearance: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val core = LocalCore.current
    val repo = id.toULong()
    val list by rememberCoreQuery { repoList() }
    val branches by rememberCoreQuery(id) { branches(repo) }
    val changes by rememberCoreQuery(id) { changes(repo) }
    var tab by rememberSaveable(id) { mutableIntStateOf(0) }
    var picker by remember { mutableStateOf(false) }
    var branchPicker by remember { mutableStateOf(false) }
    val folderPicker = rememberFolderPicker { path -> if (path != null) core.dispatch { addRepository(path) } }
    val selected = list.value?.selected
    LaunchedEffect(id, selected) {
        // after process death the stack comes back before the engine's selection does
        if (list.value != null && selected != repo) core.dispatch { selectRepository(repo) }
    }
    val info = list.value?.repositories?.firstOrNull { it.id == repo }
    Column(modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas)) {
        RepositoryTopChrome(
            repository = info?.name.orEmpty(),
            owner = info?.owner,
            branch = branches.value?.let { it.current ?: it.detachedSha?.take(SHORT_SHA) } ?: info?.branch,
            sync = syncModel(branches.value),
            onRepositoryClick = { picker = true },
            onBranchClick = { branchPicker = true },
            onSync = {},
            labels = RepositoryChromeLabels(
                currentRepository = stringResource(R.string.app_current_repository),
                currentBranch = stringResource(R.string.app_current_branch),
                noBranch = stringResource(R.string.app_no_branch),
                switchRepository = stringResource(R.string.app_switch_repository),
                switchBranch = stringResource(R.string.app_switch_branch),
                back = stringResource(R.string.app_back),
            ),
            onBack = onBack,
            repositoryAnchor = {
                val value = list.value
                if (picker && value != null) {
                    RepositoryPicker(
                        value,
                        onSelect = { chosen ->
                            picker = false
                            if (chosen.id != repo) {
                                core.dispatch { selectRepository(chosen.id) }
                                onOpenRepository(chosen.id.toLong())
                            }
                        },
                        onAdd = {
                            picker = false
                            folderPicker.pick()
                        },
                        onDismissRequest = { picker = false },
                    )
                }
            },
            branchAnchor = {
                val value = branches.value
                if (branchPicker && value != null) BranchPicker(value, onDismiss = { branchPicker = false })
            },
            actions = {
                OverflowMenu(onAppearance = onAppearance) { close ->
                    ActionMenuItem(
                        stringResource(R.string.app_refresh),
                        {
                            close()
                            core.dispatch { refreshRepository(repo) }
                        },
                        leadingIcon = Octicons.Sync,
                    )
                }
            },
            tabs = {
                UnderlineNav(
                    listOf(
                        UnderlineNavItem(stringResource(R.string.app_tab_changes), changes.value?.files?.size),
                        UnderlineNavItem(stringResource(R.string.app_tab_history)),
                    ),
                    selectedIndex = tab,
                    onSelect = { index ->
                        tab = index
                        core.dispatch { selectSection(repo, index == 1) }
                    },
                )
            },
        )
        val content = Modifier
            .weight(1f)
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
        when {
            tab == 1 -> Box(content, contentAlignment = Alignment.Center) {
                Blankslate(Octicons.History, stringResource(R.string.app_history_later))
            }
            isCompactWidth() -> ChangesRoute(id, onOpenDiff = onOpenDiff, modifier = content)
            else -> ChangesAndDiff(id, content)
        }
    }
}

/** The changes list and the diff side by side (medium and expanded widths). */
@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
private fun ChangesAndDiff(id: Long, modifier: Modifier = Modifier) {
    val navigator = rememberListDetailPaneScaffoldNavigator<Long>()
    val scope = rememberCoroutineScope()
    val expansion = rememberPaneExpansionState(anchors = PaneAnchors)
    NavigableListDetailPaneScaffold(
        navigator = navigator,
        modifier = modifier,
        listPane = {
            AnimatedPane {
                ChangesRoute(id, onOpenDiff = { scope.launch { navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, id) } })
            }
        },
        detailPane = {
            AnimatedPane {
                DiffRoute(id, contentPadding = WindowInsets.navigationBars.asPaddingValues())
            }
        },
        paneExpansionState = expansion,
        paneExpansionDragHandle = { state ->
            val interactions = remember { MutableInteractionSource() }
            VerticalDragHandle(
                modifier = Modifier.paneExpansionDraggable(state, 48.dp, interactions),
                interactionSource = interactions,
            )
        },
    )
}

private val PaneAnchors = listOf(
    PaneExpansionAnchor.Proportion(0.35f),
    PaneExpansionAnchor.Proportion(0.5f),
    PaneExpansionAnchor.Proportion(0.65f),
)

/** GHD's branch foldout, read-only until checkout arrives (M-A2): the current branch checked. */
@Composable
private fun BranchPicker(branches: BranchesVm, onDismiss: () -> Unit) {
    SelectPanel(
        title = stringResource(R.string.app_branches),
        onDismissRequest = onDismiss,
        closeDescription = stringResource(R.string.app_close),
    ) {
        items(branches.branches.filter { !it.remote }, key = { it.name }) { branch ->
            ActionListItem(
                branch.name,
                checked = branch.current,
                enabled = branch.current,
                onClick = onDismiss,
                leading = { Octicon(Octicons.GitBranch, null, tint = OcticonTint.Secondary) },
            )
        }
    }
}

/** GHD's `PushPullButton` labels from the engine's sync state; disabled until M-A3 wires the remote. */
@Composable
private fun syncModel(branches: BranchesVm?): SyncButtonModel {
    val fetched = branches?.lastFetchedAt?.let {
        stringResource(R.string.app_sync_fetched, DateUtils.getRelativeTimeSpanString(it * MILLIS, System.currentTimeMillis(), MINUTE))
    } ?: stringResource(R.string.app_sync_never)
    return when (branches?.sync) {
        null, SyncActionVm.FETCH -> SyncButtonModel(stringResource(R.string.app_sync_fetch), fetched, Octicons.Sync, enabled = false)
        SyncActionVm.PULL ->
            SyncButtonModel(stringResource(R.string.app_sync_pull, (branches.behind ?: 0u).toInt()), fetched, Octicons.ArrowDown, false)
        SyncActionVm.PUSH ->
            SyncButtonModel(stringResource(R.string.app_sync_push, (branches.ahead ?: 0u).toInt()), fetched, Octicons.ArrowUp, false)
        SyncActionVm.PUBLISH_REPOSITORY ->
            SyncButtonModel(stringResource(R.string.app_sync_publish_repository), null, Octicons.Upload, false)
        SyncActionVm.PUBLISH_BRANCH ->
            SyncButtonModel(stringResource(R.string.app_sync_publish_branch), null, Octicons.Upload, false)
        SyncActionVm.BUSY -> SyncButtonModel(
            branches.syncProgressTitle ?: stringResource(R.string.app_sync_busy),
            null,
            Octicons.Sync,
            enabled = false,
            progress = branches.syncProgress,
        )
    }
}

private const val SHORT_SHA = 7
private const val MILLIS = 1000L
private const val MINUTE = DateUtils.MINUTE_IN_MILLIS
