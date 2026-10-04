package com.wasimaster.corvene

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import com.wasimaster.corvene.changes.CommitDiffRoute
import com.wasimaster.corvene.changes.DiffRoute
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.design.PrimerTopAppBar
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.history.CommitDetailRoute
import com.wasimaster.corvene.repositories.RepositoryListRoute
import com.wasimaster.corvene.settings.AppearanceRoute

/**
 * The app's destinations on a Navigation 3 back stack: the repository list,
 * a repository ([CorveneScaffold]), the diff on its own screen (compact
 * widths), Settings › Appearance. Back (and predictive back) pops the stack.
 */
@Composable
fun CorveneNavigation(modifier: Modifier = Modifier) {
    val backStack = rememberNavBackStack(Repositories)
    val pop: () -> Unit = { backStack.removeLastOrNull() }
    NavDisplay(
        backStack = backStack,
        modifier = modifier,
        onBack = { pop() },
        entryProvider = entryProvider {
            entry<Repositories> {
                AppScaffold(
                    title = stringResource(R.string.app_name),
                    actions = { OverflowMenu(onAppearance = { backStack.add(AppearanceSettings) }) },
                ) { padding ->
                    RepositoryListRoute(onOpen = { backStack.add(Repository(it)) }, contentPadding = padding)
                }
            }
            entry<Repository> { key ->
                CorveneScaffold(
                    id = key.id,
                    onBack = pop,
                    onOpenDiff = { backStack.add(Diff(key.id)) },
                    onOpenCommit = { backStack.add(CommitDetail(key.id)) },
                    onOpenRepository = { id ->
                        // the switcher replaces the repository, it does not stack another
                        backStack.removeLastOrNull()
                        backStack.add(Repository(id))
                    },
                    onAppearance = { backStack.add(AppearanceSettings) },
                )
            }
            entry<Diff> { key ->
                val header by rememberCoreQuery(key.id) { diffHeader(key.id.toULong()) }
                val path = header.value?.path.orEmpty()
                Column(Modifier.fillMaxSize()) {
                    PrimerTopAppBar(
                        title = path.substringAfterLast('/'),
                        subtitle = path.substringBeforeLast('/', "").takeIf { it.isNotEmpty() },
                        onBack = pop,
                        backDescription = stringResource(R.string.app_back),
                    )
                    DiffRoute(
                        key.id,
                        Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal)),
                        contentPadding = WindowInsets.navigationBars.asPaddingValues(),
                    )
                }
            }
            entry<CommitDetail> { key ->
                AppScaffold(title = stringResource(R.string.app_commit_title), onBack = pop) { padding ->
                    CommitDetailRoute(
                        key.id,
                        onOpenFile = { backStack.add(CommitDiff(key.id)) },
                        modifier = Modifier.padding(top = padding.calculateTopPadding()),
                        contentPadding = WindowInsets.navigationBars.asPaddingValues(),
                    )
                }
            }
            entry<CommitDiff> { key ->
                val detail by rememberCoreQuery(key.id) { commitDetail(key.id.toULong()) }
                val path = detail.value?.selectedFile.orEmpty()
                Column(Modifier.fillMaxSize()) {
                    PrimerTopAppBar(
                        title = path.substringAfterLast('/'),
                        subtitle = path.substringBeforeLast('/', "").takeIf { it.isNotEmpty() },
                        onBack = pop,
                        backDescription = stringResource(R.string.app_back),
                    )
                    CommitDiffRoute(
                        key.id,
                        Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal)),
                        contentPadding = WindowInsets.navigationBars.asPaddingValues(),
                    )
                }
            }
            entry<AppearanceSettings> {
                AppScaffold(
                    title = stringResource(R.string.app_appearance),
                    subtitle = stringResource(R.string.app_settings),
                    onBack = pop,
                ) { padding -> AppearanceRoute(contentPadding = padding) }
            }
        },
    )
}

/** The top bar every plain destination shares, over the canvas colour. */
@Composable
internal fun AppScaffold(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    actions: @Composable () -> Unit = {},
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        modifier = modifier,
        containerColor = CorveneTheme.colors.bgCanvas,
        topBar = {
            PrimerTopAppBar(
                title = title,
                subtitle = subtitle,
                onBack = onBack,
                backDescription = stringResource(R.string.app_back),
                actions = { actions() },
            )
        },
        content = content,
    )
}

/** The bar's overflow menu: Settings › Appearance for now (the temporary paintbrush menu is gone). */
@Composable
internal fun OverflowMenu(onAppearance: () -> Unit, extra: @Composable (close: () -> Unit) -> Unit = {}) {
    var open by remember { mutableStateOf(false) }
    Box {
        PrimerIconButton(Octicons.KebabHorizontal, stringResource(R.string.app_more), { open = true })
        ActionMenu(expanded = open, onDismissRequest = { open = false }) {
            extra { open = false }
            ActionMenuItem(
                stringResource(R.string.app_appearance),
                {
                    open = false
                    onAppearance()
                },
                leadingIcon = Octicons.Paintbrush,
            )
        }
    }
}
