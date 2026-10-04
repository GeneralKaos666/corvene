package com.wasimaster.corvene

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
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
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.history.CommitDetailRoute
import com.wasimaster.corvene.onboarding.SignInRoute
import com.wasimaster.corvene.repositories.AddRepositoryRoute
import com.wasimaster.corvene.repositories.CloneProgressRoute
import com.wasimaster.corvene.repositories.CloneRoute
import com.wasimaster.corvene.repositories.CreateRepositoryRoute
import com.wasimaster.corvene.repositories.RepositoryListRoute
import com.wasimaster.corvene.design.isCompactWidth
import com.wasimaster.corvene.settings.LicensesRoute
import com.wasimaster.corvene.settings.SettingsListScreen
import com.wasimaster.corvene.settings.SettingsPanes
import com.wasimaster.corvene.settings.SettingsSection
import com.wasimaster.corvene.settings.SettingsSectionRoute
import androidx.navigation3.runtime.NavKey

/**
 * The app's destinations on a Navigation 3 back stack: the repository list,
 * a repository ([CorveneScaffold]), the diff on its own screen (compact
 * widths), Settings › Appearance. Back (and predictive back) pops the stack.
 */
@Composable
fun CorveneNavigation(modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val backStack = rememberNavBackStack(Repositories)
    val pop: () -> Unit = { backStack.removeLastOrNull() }
    // a clone that finished opens its repository (GHD selects it)
    CloneProgressRoute(onCloned = { id ->
        if (backStack.lastOrNull() != Repository(id)) backStack.add(Repository(id))
    })
    NavDisplay(
        backStack = backStack,
        modifier = modifier,
        onBack = { pop() },
        entryProvider = entryProvider {
            entry<Repositories> {
                AppScaffold(
                    title = stringResource(R.string.app_name),
                    actions = {
                        AddMenu(
                            onClone = { backStack.add(Clone()) },
                            onCreate = { backStack.add(CreateRepository) },
                            onAdd = { backStack.add(AddRepository) },
                        )
                        OverflowMenu(onSettings = { backStack.add(Settings) })
                    },
                ) { padding ->
                    RepositoryListRoute(
                        onOpen = { backStack.add(Repository(it)) },
                        onClone = { backStack.add(Clone()) },
                        onCreate = { backStack.add(CreateRepository) },
                        onAddExisting = { backStack.add(AddRepository) },
                        contentPadding = padding,
                    )
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
                    onSettings = { backStack.add(Settings) },
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
            entry<Clone> { key ->
                CloneRoute(key.url, onClose = pop, onSignIn = { backStack.add(SignIn(enterprise = false)) })
            }
            entry<AddRepository> { AddRepositoryRoute(onClose = pop) }
            entry<CreateRepository> { CreateRepositoryRoute(onClose = pop) }
            entry<SignIn> { key ->
                AppScaffold(
                    title = stringResource(if (key.enterprise) R.string.app_sign_in_enterprise else R.string.app_sign_in),
                    onBack = {
                        core.dispatch { cancelSignIn() }
                        pop()
                    },
                ) { padding -> SignInRoute(key.enterprise, onSignedIn = pop, contentPadding = padding) }
            }
            entry<Settings> {
                SettingsDestination(
                    section = null,
                    onBack = pop,
                    onOpen = { section -> backStack.add(SettingsPage(section.key)) },
                    onReplace = { section -> backStack.replaceLast(SettingsPage(section.key)) },
                    onSignIn = { enterprise -> backStack.add(SignIn(enterprise)) },
                    onLicenses = { backStack.add(Licenses) },
                )
            }
            entry<SettingsPage> { key ->
                SettingsDestination(
                    section = SettingsSection.fromKey(key.section),
                    onBack = pop,
                    onOpen = { section -> backStack.add(SettingsPage(section.key)) },
                    onReplace = { section -> backStack.replaceLast(SettingsPage(section.key)) },
                    onSignIn = { enterprise -> backStack.add(SignIn(enterprise)) },
                    onLicenses = { backStack.add(Licenses) },
                )
            }
            entry<Licenses> {
                AppScaffold(
                    title = stringResource(R.string.app_licenses),
                    subtitle = stringResource(R.string.app_settings),
                    onBack = pop,
                ) { padding -> LicensesRoute(contentPadding = padding) }
            }
        },
    )
    PopupHost(
        onSignIn = { enterprise -> backStack.add(SignIn(enterprise)) },
        onOpenSettings = { section -> backStack.add(SettingsSection.fromKey(section)?.let { SettingsPage(it.key) } ?: Settings) },
    )
    JankScreenTag(backStack.lastOrNull())
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

/** The "+" menu (GHD's File menu): Clone, Create, Add existing. */
@Composable
internal fun AddMenu(onClone: () -> Unit, onCreate: () -> Unit, onAdd: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        PrimerIconButton(Octicons.Plus, stringResource(R.string.app_add_menu), { open = true })
        ActionMenu(expanded = open, onDismissRequest = { open = false }) {
            ActionMenuItem(
                stringResource(R.string.app_clone),
                {
                    open = false
                    onClone()
                },
                leadingIcon = Octicons.Download,
            )
            ActionMenuItem(
                stringResource(R.string.app_create),
                {
                    open = false
                    onCreate()
                },
                leadingIcon = Octicons.Plus,
            )
            ActionMenuItem(
                stringResource(R.string.app_add_existing),
                {
                    open = false
                    onAdd()
                },
                leadingIcon = Octicons.FileDirectory,
            )
        }
    }
}

/** The bar's overflow menu: [extra] items, then Settings. */
@Composable
internal fun OverflowMenu(onSettings: () -> Unit, extra: @Composable (close: () -> Unit) -> Unit = {}) {
    var open by remember { mutableStateOf(false) }
    Box {
        PrimerIconButton(Octicons.KebabHorizontal, stringResource(R.string.app_more), { open = true })
        ActionMenu(expanded = open, onDismissRequest = { open = false }) {
            extra { open = false }
            ActionMenuItem(
                stringResource(R.string.app_settings),
                {
                    open = false
                    onSettings()
                },
                leadingIcon = Octicons.Gear,
            )
        }
    }
}

/**
 * Settings as a destination: on compact widths the sections list
 * ([section] null) or one section under its own bar, on wider ones the list
 * beside the section ([section], else Accounts), where picking another
 * section replaces this entry ([onReplace]) instead of stacking one.
 */
@Composable
private fun SettingsDestination(
    section: SettingsSection?,
    onBack: () -> Unit,
    onOpen: (SettingsSection) -> Unit,
    onReplace: (SettingsSection) -> Unit,
    onSignIn: (enterprise: Boolean) -> Unit,
    onLicenses: () -> Unit,
) {
    val version = "${BuildConfig.VERSION_NAME} (${BuildConfig.FLAVOR}, ${BuildConfig.BUILD_TYPE})"
    val compact = isCompactWidth()
    val title = if (compact && section != null) stringResource(section.title) else stringResource(R.string.app_settings)
    AppScaffold(
        title = title,
        subtitle = if (compact && section != null) stringResource(R.string.app_settings) else null,
        onBack = onBack,
    ) { padding ->
        val content = Modifier.padding(top = padding.calculateTopPadding())
        val bottom = WindowInsets.navigationBars.asPaddingValues()
        when {
            !compact -> SettingsPanes(
                selected = section ?: SettingsSection.Accounts,
                onSelect = { if (it != section) onReplace(it) },
                modifier = content.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal)),
                contentPadding = bottom,
            ) { shown -> SettingsSectionRoute(shown, version, onSignIn, onLicenses, contentPadding = bottom) }
            section == null -> SettingsListScreen(onOpen = onOpen, modifier = content, contentPadding = bottom)
            else -> SettingsSectionRoute(section, version, onSignIn, onLicenses, content, contentPadding = bottom)
        }
    }
}

/** Swaps the top entry (a section picked beside the list replaces the shown one). */
private fun <T : NavKey> MutableList<T>.replaceLast(key: T) {
    if (isNotEmpty()) removeAt(lastIndex)
    add(key)
}
