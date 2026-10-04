package com.wasimaster.corvene

import android.app.Activity
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.VerticalDragHandle
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.layout.AnimatedPane
import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffoldRole
import androidx.compose.material3.adaptive.layout.PaneExpansionAnchor
import androidx.compose.material3.adaptive.layout.rememberPaneExpansionState
import androidx.compose.material3.adaptive.navigation.NavigableListDetailPaneScaffold
import androidx.compose.material3.adaptive.navigation.rememberListDetailPaneScaffoldNavigator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.branches.BranchSheetRoute
import com.wasimaster.corvene.branches.CreateBranchDialog
import com.wasimaster.corvene.branches.model
import com.wasimaster.corvene.branches.rememberSyncController
import com.wasimaster.corvene.changes.ChangesRoute
import com.wasimaster.corvene.changes.CommitDiffRoute
import com.wasimaster.corvene.changes.DiffRoute
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.KeyCommand
import com.wasimaster.corvene.design.KeyCommands
import com.wasimaster.corvene.design.LocalKeyCommands
import com.wasimaster.corvene.design.RailItem
import com.wasimaster.corvene.design.RepositoryRail
import com.wasimaster.corvene.design.isExpandedWidth
import com.wasimaster.corvene.design.usesNavigationRail
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.RepositoryChromeLabels
import com.wasimaster.corvene.design.RepositoryTopChrome
import com.wasimaster.corvene.design.UnderlineNav
import com.wasimaster.corvene.design.UnderlineNavItem
import com.wasimaster.corvene.design.isCompactWidth
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.history.CommitDetailRoute
import com.wasimaster.corvene.history.HistoryRoute
import com.wasimaster.corvene.platform.OpenPath
import com.wasimaster.corvene.platform.Termux
import com.wasimaster.corvene.platform.rememberFolderPicker
import com.wasimaster.corvene.repositories.RepositoryPicker
import kotlinx.coroutines.launch

/**
 * A repository: the chrome of the style in effect ([RepositoryTopChrome]:
 * the repository switcher, the branch panel, the sync button with its menu,
 * the Changes | History tabs), the engine's banner, then the tab. Compact
 * widths show one pane and push the diff / the commit as their own
 * destinations ([onOpenDiff], [onOpenCommit]); medium and expanded widths put
 * the list and its detail side by side in a list-detail scaffold with a
 * draggable divider (GHD's 250 dp sidebar to start with on expanded ones).
 * GitHub Mobile and Material navigate with a [RepositoryRail] on medium and
 * expanded widths; GitHub Desktop keeps the Changes | History bar, over the
 * sidebar when there are two panes. A hardware keyboard has GHD's
 * accelerators ([Shortcut]); the ones a screen owns (commit, filter) reach it
 * through [LocalKeyCommands]. The only place that touches adaptive navigation.
 */
@Composable
fun CorveneScaffold(
    id: Long,
    onBack: () -> Unit,
    onOpenDiff: () -> Unit,
    onOpenCommit: () -> Unit,
    onOpenRepository: (Long) -> Unit,
    onSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val core = LocalCore.current
    val repo = id.toULong()
    val list by rememberCoreQuery { repoList() }
    val branches by rememberCoreQuery(id) { branches(repo) }
    val changes by rememberCoreQuery(id) { changes(repo) }
    val banner by rememberCoreQuery { banner() }
    var tab by rememberSaveable(id) { mutableIntStateOf(0) }
    var picker by remember { mutableStateOf(false) }
    var branchPicker by remember { mutableStateOf(false) }
    var branchFrom by rememberSaveable { mutableStateOf<String?>(null) }
    var newBranch by rememberSaveable { mutableStateOf(false) }
    val keyCommands = remember { KeyCommands() }
    val focus = remember { FocusRequester() }
    val folderPicker = rememberFolderPicker { path -> if (path != null) core.dispatch { addRepository(path) } }
    val context = LocalContext.current
    val termux = remember(context) { Termux.installed(context) }
    val selected = list.value?.selected
    LaunchedEffect(id, selected) {
        // after process death the stack comes back before the engine's selection does
        if (list.value != null && selected != repo) core.dispatch { selectRepository(repo) }
    }
    val info = list.value?.repositories?.firstOrNull { it.id == repo }
    val sync = rememberSyncController(id, branches.value, info?.name.orEmpty())
    val selectTab: (Int) -> Unit = { index ->
        tab = index
        core.dispatch { selectSection(repo, index == 1) }
    }
    val rail = usesNavigationRail()
    val compact = isCompactWidth()
    val onShortcut: (Shortcut) -> Boolean = { shortcut ->
        when (shortcut) {
            Shortcut.Changes -> true.also { selectTab(0) }
            Shortcut.History -> true.also { selectTab(1) }
            Shortcut.Repositories -> true.also { picker = true }
            Shortcut.Branches -> true.also { branchPicker = true }
            Shortcut.Push -> true.also { sync.push() }
            Shortcut.Pull -> true.also { sync.pull() }
            Shortcut.Fetch -> true.also { sync.fetch() }
            Shortcut.NewBranch -> true.also { newBranch = true }
            Shortcut.Commit -> {
                if (tab != 0) selectTab(0)
                keyCommands.send(KeyCommand.Commit)
            }
            Shortcut.Filter -> keyCommands.send(KeyCommand.Filter)
            Shortcut.Close -> (picker || branchPicker).also {
                picker = false
                branchPicker = false
            }
        }
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val tabs: @Composable () -> Unit = {
        UnderlineNav(
            listOf(
                UnderlineNavItem(stringResource(R.string.app_tab_changes), changes.value?.files?.size),
                UnderlineNavItem(stringResource(R.string.app_tab_history)),
            ),
            selectedIndex = tab,
            onSelect = selectTab,
        )
    }
    Row(
        modifier
            .fillMaxSize()
            .background(CorveneTheme.colors.bgCanvas)
            .onPreviewKeyEvent { event -> Shortcut.of(event)?.let(onShortcut) ?: false }
            .focusRequester(focus)
            .focusTarget(),
    ) {
        if (rail) {
            RepositoryRail(
                listOf(
                    RailItem(stringResource(R.string.app_tab_changes), Octicons.FileDiff, changes.value?.files?.size, "app_rail_changes"),
                    RailItem(stringResource(R.string.app_tab_history), Octicons.History, tag = "app_rail_history"),
                    RailItem(stringResource(R.string.app_rail_branches), Octicons.GitBranch, tag = "app_rail_branches"),
                    RailItem(stringResource(R.string.app_rail_repositories), Octicons.Repo, tag = "app_rail_repositories"),
                ),
                selectedIndex = tab,
                onSelect = { index ->
                    when (index) {
                        0, 1 -> selectTab(index)
                        2 -> branchPicker = true
                        else -> picker = true
                    }
                },
                modifier = Modifier.windowInsetsPadding(
                    WindowInsets.safeDrawing.only(WindowInsetsSides.Vertical + WindowInsetsSides.Start),
                ),
            )
        }
        Column(Modifier.weight(1f).fillMaxHeight()) {
            RepositoryTopChrome(
                repository = info?.name.orEmpty(),
                owner = info?.owner,
                branch = branches.value?.let { it.current ?: it.detachedSha?.take(SHORT_SHA) } ?: info?.branch,
                sync = sync.model(),
                onRepositoryClick = { picker = true },
                onBranchClick = { branchPicker = true },
                onSync = sync::click,
                onSyncLongClick = sync::longClick,
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
                    if (branchPicker) {
                        BranchSheetRoute(
                            id,
                            github = info?.github != null,
                            onDismissRequest = { branchPicker = false },
                            onCompare = { selectTab(1) },
                        )
                    }
                },
                syncAnchor = { sync.Menu() },
                actions = {
                    OverflowMenu(onSettings = onSettings) { close ->
                        ActionMenuItem(
                            stringResource(R.string.app_refresh),
                            {
                                close()
                                core.dispatch { refreshRepository(repo) }
                            },
                            leadingIcon = Octicons.Sync,
                        )
                        ActionMenuItem(
                            stringResource(R.string.app_repository_settings),
                            {
                                close()
                                core.dispatch { openRepositorySettings(repo, "remote") }
                            },
                            leadingIcon = Octicons.Gear,
                        )
                        val path = info?.path
                        if (path != null) {
                            ActionMenuItem(
                                stringResource(R.string.app_show_in_files),
                                {
                                    close()
                                    OpenPath.open(context, path, reveal = false)?.let { toast -> showToast(context, toast) }
                                },
                                leadingIcon = Octicons.FileDirectory,
                            )
                            if (termux) {
                                ActionMenuItem(
                                    stringResource(R.string.app_open_in_termux),
                                    {
                                        close()
                                        val activity = context as? Activity
                                        activity?.let { Termux.open(it, path) }?.let { toast -> showToast(context, toast) }
                                    },
                                    leadingIcon = Octicons.CodeSquare,
                                )
                            }
                        }
                    }
                },
                // compact: under the chrome; the rail replaces them; GHD's wider layout puts them over the sidebar
                tabs = { if (compact) tabs() },
            )
            BannerFlash(banner.value, onViewConflicts = { core.dispatch { showConflicts(repo) } })
            val content = Modifier
                .weight(1f)
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
            val createBranchFrom: (String) -> Unit = { branchFrom = it }
            val sidebarTabs: (@Composable () -> Unit)? = if (!compact && !rail) tabs else null
            CompositionLocalProvider(LocalKeyCommands provides keyCommands) {
                when {
                    tab == 1 && compact -> HistoryRoute(
                        id,
                        github = info?.github,
                        onOpenCommit = onOpenCommit,
                        onCreateBranchFrom = createBranchFrom,
                        modifier = content,
                        contentPadding = WindowInsets.navigationBars.asPaddingValues(),
                    )
                    tab == 1 -> HistoryAndCommit(id, info?.github, createBranchFrom, sidebarTabs, content)
                    compact -> ChangesRoute(id, onOpenDiff = onOpenDiff, modifier = content)
                    else -> ChangesAndDiff(id, sidebarTabs, content)
                }
            }
        }
    }
    sync.Dialogs()
    if (newBranch) {
        val value = branches.value
        CreateBranchDialog(
            initialName = "",
            current = value?.current,
            defaultBranch = value?.defaultBranch,
            targetSha = null,
            existing = value?.branches?.map { it.name }?.toSet().orEmpty(),
            onCreate = { name, start ->
                newBranch = false
                core.dispatch { createBranch(repo, name, start) }
            },
            onDismissRequest = { newBranch = false },
        )
    }
    branchFrom?.let { sha ->
        val value = branches.value
        CreateBranchDialog(
            initialName = "",
            current = value?.current,
            defaultBranch = value?.defaultBranch,
            targetSha = sha,
            existing = value?.branches?.map { it.name }?.toSet().orEmpty(),
            onCreate = { name, start ->
                branchFrom = null
                core.dispatch { createBranch(repo, name, start) }
            },
            onDismissRequest = { branchFrom = null },
        )
    }
}

/** The changes list and the diff side by side (medium and expanded widths), [tabs] over the list (GHD's sidebar). */
@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
private fun ChangesAndDiff(id: Long, tabs: (@Composable () -> Unit)?, modifier: Modifier = Modifier) {
    ListAndDetail(
        list = { open -> ChangesRoute(id, onOpenDiff = open) },
        detail = { DiffRoute(id, contentPadding = WindowInsets.navigationBars.asPaddingValues()) },
        tabs = tabs,
        modifier = modifier,
    )
}

/** History and the selected commit (its files over the file's diff) side by side. */
@Composable
private fun HistoryAndCommit(
    id: Long,
    github: String?,
    onCreateBranchFrom: (String) -> Unit,
    tabs: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
) {
    ListAndDetail(
        modifier = modifier,
        tabs = tabs,
        list = { open -> HistoryRoute(id, github, onOpenCommit = open, onCreateBranchFrom = onCreateBranchFrom) },
        detail = {
            Column(Modifier.fillMaxSize()) {
                CommitDetailRoute(id, onOpenFile = {}, modifier = Modifier.weight(DETAIL_FILES))
                HorizontalDivider(color = CorveneTheme.colors.borderMuted)
                CommitDiffRoute(
                    id,
                    Modifier.weight(1f - DETAIL_FILES),
                    contentPadding = WindowInsets.navigationBars.asPaddingValues(),
                )
            }
        },
    )
}

/**
 * A list-detail scaffold with a draggable divider: on expanded widths the
 * list starts as GHD's 250 dp sidebar, on medium ones at half; the divider
 * snaps to 250 dp, 35, 50 or 65 %. [tabs] (GHD's Changes | History bar)
 * sit over the list when the chrome does not carry them.
 */
@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
private fun ListAndDetail(
    list: @Composable (open: () -> Unit) -> Unit,
    detail: @Composable () -> Unit,
    tabs: (@Composable () -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val navigator = rememberListDetailPaneScaffoldNavigator<Long>()
    val scope = rememberCoroutineScope()
    val expanded = isExpandedWidth()
    val expansion = rememberPaneExpansionState(anchors = PaneAnchors, initialAnchoredIndex = if (expanded) SIDEBAR_ANCHOR else HALF_ANCHOR)
    NavigableListDetailPaneScaffold(
        navigator = navigator,
        modifier = modifier,
        listPane = {
            AnimatedPane {
                Column(Modifier.fillMaxSize()) {
                    tabs?.invoke()
                    Box(Modifier.weight(1f)) {
                        list { scope.launch { navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, 0L) } }
                    }
                }
            }
        },
        detailPane = { AnimatedPane { detail() } },
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

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
private val PaneAnchors = listOf(
    PaneExpansionAnchor.Offset.fromStart(250.dp),
    PaneExpansionAnchor.Proportion(0.35f),
    PaneExpansionAnchor.Proportion(0.5f),
    PaneExpansionAnchor.Proportion(0.65f),
)

private const val SHORT_SHA = 7
private const val DETAIL_FILES = 0.4f
private const val SIDEBAR_ANCHOR = 0
private const val HALF_ANCHOR = 2

private fun showToast(context: Context, message: String) {
    Toast.makeText(context, message, Toast.LENGTH_LONG).show()
}
