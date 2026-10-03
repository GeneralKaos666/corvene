package com.wasimaster.corvene

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.CounterLabel
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.ffi.gen.ThemeVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.repositories.RepositoryListRoute

/** The app's destinations on a Navigation 3 back stack. */
@Composable
fun CorveneNavigation(
    appearance: Appearance,
    onStyle: (DesignStyle) -> Unit,
    onTheme: (ThemeVm) -> Unit,
    modifier: Modifier = Modifier,
) {
    val backStack = rememberNavBackStack(Repositories)
    NavDisplay(
        backStack = backStack,
        modifier = modifier,
        onBack = { backStack.removeLastOrNull() },
        entryProvider = entryProvider {
            entry<Repositories> {
                AppScaffold(
                    title = stringResource(R.string.app_name),
                    actions = { AppearanceMenu(appearance, onStyle, onTheme) },
                ) { padding ->
                    RepositoryListRoute(onOpen = { backStack.add(Repository(it)) }, contentPadding = padding)
                }
            }
            entry<Repository> { key ->
                RepositoryPlaceholder(key.id, onBack = { backStack.removeLastOrNull() })
            }
        },
    )
}

/** The top bar every destination shares: Primer chrome colours, Octicon actions. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AppScaffold(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    actions: @Composable () -> Unit = {},
    content: @Composable (PaddingValues) -> Unit,
) {
    val colors = CorveneTheme.colors
    Scaffold(
        modifier = modifier,
        containerColor = colors.bgCanvas,
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        if (subtitle != null) {
                            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = colors.toolbarTextSecondary, maxLines = 1)
                        }
                        Text(title, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                },
                navigationIcon = {
                    if (onBack != null) {
                        PrimerIconButton(Octicons.ArrowLeft, stringResource(R.string.app_back), onBack, tint = OcticonTint.Link)
                    }
                },
                actions = { actions() },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = colors.toolbarBg,
                    titleContentColor = colors.toolbarText,
                    actionIconContentColor = colors.toolbarText,
                    navigationIconContentColor = colors.toolbarText,
                ),
            )
        },
        content = content,
    )
}

/**
 * Temporary: the design style and theme (the engine's settings) in the top
 * bar's menu. Moves to Settings › Appearance (style cards) with M-A1.
 */
@Composable
private fun AppearanceMenu(appearance: Appearance, onStyle: (DesignStyle) -> Unit, onTheme: (ThemeVm) -> Unit) {
    var open by remember { mutableStateOf(false) }
    PrimerIconButton(Octicons.Paintbrush, stringResource(R.string.app_appearance), { open = true }, tint = OcticonTint.Link)
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        listOf(
            DesignStyle.GitHubMobile to R.string.app_style_github_mobile,
            DesignStyle.GitHubDesktop to R.string.app_style_github_desktop,
            DesignStyle.Material to R.string.app_style_material,
        ).forEach { (style, label) ->
            DropdownMenuItem(
                text = { Text(stringResource(label)) },
                onClick = {
                    open = false
                    onStyle(style)
                },
                // a style pinned by flag 112 shows what is drawn; the choice is kept for later
                enabled = !appearance.stylePinned,
                trailingIcon = { if (style == appearance.styleSetting) Octicon(Octicons.Check, null) },
            )
        }
        HorizontalDivider()
        listOf(
            ThemeVm.SYSTEM to R.string.app_mode_system,
            ThemeVm.LIGHT to R.string.app_mode_light,
            ThemeVm.DARK to R.string.app_mode_dark,
            ThemeVm.HIGH_CONTRAST to R.string.app_mode_high_contrast,
        ).forEach { (theme, label) ->
            DropdownMenuItem(
                text = { Text(stringResource(label)) },
                onClick = {
                    open = false
                    onTheme(theme)
                },
                trailingIcon = { if (theme == appearance.theme) Octicon(Octicons.Check, null) },
            )
        }
    }
}

/** M-A0's stand-in for the repository screens: the selected repository's name, branch and indicators. */
@Composable
private fun RepositoryPlaceholder(id: Long, onBack: () -> Unit) {
    val list by rememberCoreQuery { repoList() }
    val repo = list.value?.repositories?.firstOrNull { it.id.toLong() == id }
    val colors = CorveneTheme.colors
    AppScaffold(title = repo?.name.orEmpty(), subtitle = repo?.github, onBack = onBack) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(CorveneTheme.metrics.gutter),
            verticalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.m),
        ) {
            if (repo == null) {
                if (!list.loading) Text(stringResource(R.string.app_repository_missing), color = colors.textSecondary)
                return@Column
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.s)) {
                Octicon(Octicons.GitBranch, null, tint = OcticonTint.Secondary)
                Text(
                    repo.branch ?: stringResource(R.string.app_no_branch),
                    style = CorveneTheme.textStyles.code,
                    color = colors.textPrimary,
                )
                repo.ahead?.takeIf { it > 0u }?.let { CounterLabel("↑$it") }
                repo.behind?.takeIf { it > 0u }?.let { CounterLabel("↓$it") }
            }
            Text(repo.path, style = CorveneTheme.textStyles.codeSmall, color = colors.textSecondary)
            Text(
                stringResource(R.string.app_repository_placeholder),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.textSecondary,
            )
        }
    }
}
