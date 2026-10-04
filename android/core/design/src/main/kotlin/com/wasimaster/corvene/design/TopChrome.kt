package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * A screen's top bar: [title] (with a small [subtitle] over it, GitHub
 * Mobile's breadcrumb line), a back arrow when [onBack] is set, [actions] in
 * link blue (Primer Mobile's chrome rule). GitHub Desktop draws it on the
 * toolbar colour with GHD's smaller type.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PrimerTopAppBar(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    backDescription: String = "",
    actions: @Composable RowScope.() -> Unit = {},
) {
    val colors = CorveneTheme.colors
    val style = LocalDesignStyle.current
    CompositionLocalProvider(LocalOcticonTint provides OcticonTint.Link) {
        TopAppBar(
            modifier = modifier,
            title = {
                Column {
                    if (subtitle != null) {
                        Text(subtitle, style = MaterialTheme.typography.bodySmall, color = colors.toolbarTextSecondary, maxLines = 1)
                    }
                    Text(
                        title,
                        style = if (style == DesignStyle.GitHubDesktop) {
                            MaterialTheme.typography.titleMedium
                        } else {
                            MaterialTheme.typography.titleLarge
                        },
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            },
            navigationIcon = {
                if (onBack != null) PrimerIconButton(Octicons.ArrowLeft, backDescription, onBack, tint = OcticonTint.Link)
            },
            actions = actions,
            colors = barColors(),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun barColors() = if (LocalDesignStyle.current == DesignStyle.Material) {
    TopAppBarDefaults.topAppBarColors()
} else {
    val colors = CorveneTheme.colors
    TopAppBarDefaults.topAppBarColors(
        containerColor = colors.toolbarBg,
        titleContentColor = colors.toolbarText,
        actionIconContentColor = colors.toolbarText,
        navigationIconContentColor = colors.toolbarText,
    )
}

/** What the push/pull button says (GHD `PushPullButton`): "Fetch origin", "Pull 2", "Publish branch". */
@Immutable
data class SyncButtonModel(
    val title: String,
    val description: String?,
    val icon: OcticonIcon,
    val enabled: Boolean,
    val progress: Float? = null,
)

/**
 * The repository's chrome. GitHub Mobile and Material: a top app bar whose
 * title is the repository switcher ([repository] + chevron →
 * [onRepositoryClick]), then the branch chip and the sync button in a row,
 * then [tabs]. GitHub Desktop: GHD's toolbar (Current repository ▾ |
 * Current branch ▾ | push/pull) on the toolbar colour, then [tabs].
 * [repositoryAnchor] and [branchAnchor] draw inside the triggers' boxes,
 * where anchored pickers (popups on wide screens) belong.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RepositoryTopChrome(
    repository: String,
    branch: String?,
    sync: SyncButtonModel,
    onRepositoryClick: () -> Unit,
    onBranchClick: () -> Unit,
    onSync: () -> Unit,
    labels: RepositoryChromeLabels,
    modifier: Modifier = Modifier,
    owner: String? = null,
    onBack: (() -> Unit)? = null,
    repositoryAnchor: @Composable () -> Unit = {},
    branchAnchor: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    tabs: @Composable () -> Unit = {},
) {
    val colors = CorveneTheme.colors
    if (LocalDesignStyle.current == DesignStyle.GitHubDesktop) {
        Column(modifier.fillMaxWidth()) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .background(colors.toolbarBg)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
                    .height(TOOLBAR_HEIGHT.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (onBack != null) {
                    PrimerIconButton(Octicons.ArrowLeft, labels.back, onBack, tint = OcticonTint.Primary)
                }
                ToolbarButton(
                    Octicons.Repo,
                    labels.currentRepository,
                    repository,
                    true,
                    onRepositoryClick,
                    Modifier.weight(1f),
                    repositoryAnchor,
                )
                ToolbarDivider()
                ToolbarButton(
                    Octicons.GitBranch,
                    labels.currentBranch,
                    branch ?: labels.noBranch,
                    true,
                    onBranchClick,
                    Modifier.weight(1f),
                    branchAnchor,
                )
                ToolbarDivider()
                ToolbarButton(sync.icon, sync.title, sync.description.orEmpty(), false, onSync, Modifier.weight(1f), enabled = sync.enabled)
                CompositionLocalProvider(LocalOcticonTint provides OcticonTint.Primary) { Row(content = actions) }
            }
            tabs()
        }
        return
    }
    val barBg = if (LocalDesignStyle.current == DesignStyle.Material) MaterialTheme.colorScheme.surface else colors.toolbarBg
    Column(modifier.fillMaxWidth().background(barBg)) {
        CompositionLocalProvider(LocalOcticonTint provides OcticonTint.Link) {
            TopAppBar(
                title = {
                    Box {
                        Row(
                            Modifier.clickable(role = Role.Button, onClickLabel = labels.switchRepository, onClick = onRepositoryClick),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            RepositoryTitle(owner, repository, Modifier.weight(1f, fill = false))
                            Octicon(Octicons.ChevronDown, null, tint = OcticonTint.Secondary)
                        }
                        repositoryAnchor()
                    }
                },
                navigationIcon = {
                    if (onBack != null) PrimerIconButton(Octicons.ArrowLeft, labels.back, onBack, tint = OcticonTint.Link)
                },
                actions = actions,
                colors = barColors(),
            )
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter).padding(bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.weight(1f, fill = false)) {
                BranchChip(branch ?: labels.noBranch, onBranchClick, labels.switchBranch)
                branchAnchor()
            }
            Box(Modifier.weight(1f))
            PrimerButton(
                sync.title,
                onSync,
                enabled = sync.enabled,
                leadingIcon = sync.icon,
            )
        }
        tabs()
    }
}

/** The words the chrome draws, from the caller's string resources. */
@Immutable
data class RepositoryChromeLabels(
    val currentRepository: String,
    val currentBranch: String,
    val noBranch: String,
    val switchRepository: String,
    val switchBranch: String,
    val back: String,
)

@Composable
private fun RepositoryTitle(owner: String?, repository: String, modifier: Modifier) {
    Column(modifier) {
        if (owner != null) {
            Text(owner, style = MaterialTheme.typography.bodySmall, color = CorveneTheme.colors.toolbarTextSecondary, maxLines = 1)
        }
        Text(repository, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun BranchChip(name: String, onClick: () -> Unit, clickLabel: String) {
    val colors = CorveneTheme.colors
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(50),
        color = colors.bgSubtle,
        contentColor = colors.textPrimary,
        border = BorderStroke(1.dp, colors.borderMuted),
        modifier = Modifier.height(36.dp),
    ) {
        Row(
            Modifier.padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Octicon(Octicons.GitBranch, clickLabel, tint = OcticonTint.Secondary)
            Text(
                name,
                style = CorveneTheme.textStyles.code,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
                modifier = Modifier.widthIn(max = BRANCH_MAX_WIDTH.dp),
            )
            Octicon(Octicons.TriangleDown, null, tint = OcticonTint.Secondary)
        }
    }
}

@Composable
private fun ToolbarButton(
    icon: OcticonIcon,
    label: String,
    value: String,
    dropdown: Boolean,
    onClick: () -> Unit,
    modifier: Modifier,
    anchor: @Composable () -> Unit = {},
    enabled: Boolean = true,
) {
    val colors = CorveneTheme.colors
    Box(modifier.fillMaxHeight()) {
        Row(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth()
                .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                .padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OcticonColored(icon, null, if (enabled) colors.toolbarText else colors.toolbarTextSecondary)
            Column(Modifier.weight(1f)) {
                Text(label, style = MaterialTheme.typography.labelSmall, color = colors.toolbarTextSecondary, maxLines = 1)
                Text(
                    value,
                    style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = if (enabled) colors.toolbarText else colors.toolbarTextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (dropdown) OcticonColored(Octicons.TriangleDown, null, colors.toolbarTextSecondary)
        }
        anchor()
    }
}

@Composable
private fun ToolbarDivider() {
    VerticalDivider(Modifier.fillMaxHeight().width(1.dp), color = CorveneTheme.colors.toolbarButtonBorder)
}

private const val TOOLBAR_HEIGHT = 50
private const val BRANCH_MAX_WIDTH = 180

internal val PreviewChromeLabels = RepositoryChromeLabels(
    currentRepository = "Current repository",
    currentBranch = "Current branch",
    noBranch = "No branch",
    switchRepository = "Switch repository",
    switchBranch = "Switch branch",
    back = "Back",
)

@Preview(widthDp = 360, heightDp = 800)
@Composable
private fun RepositoryTopChromePreview() {
    DesignStyleSamples {
        RepositoryTopChrome(
            repository = "corvene",
            owner = "wasi-master",
            branch = "main",
            sync = SyncButtonModel("Fetch origin", "Never fetched", Octicons.Sync, enabled = false),
            onRepositoryClick = {},
            onBranchClick = {},
            onSync = {},
            labels = PreviewChromeLabels,
            tabs = { UnderlineNav(listOf(UnderlineNavItem("Changes", 2), UnderlineNavItem("History")), 0, {}) },
        )
        PrimerTopAppBar("Appearance", subtitle = "Settings", onBack = {})
    }
}

/**
 * A miniature of the style in effect (Settings › Appearance's cards): its
 * top bar with [repository], the tab underline, two file rows and its primary
 * button saying [button]. Drawing only; the card around it takes the tap.
 */
@Composable
fun StyleMiniature(repository: String, button: String, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    val style = LocalDesignStyle.current
    val corner = RoundedCornerShape(CorveneTheme.metrics.cornerMedium)
    Column(
        modifier
            .fillMaxWidth()
            .height(MINIATURE_HEIGHT.dp)
            .clip(corner)
            .background(colors.bgCanvas)
            .border(1.dp, colors.borderMuted, corner),
    ) {
        Row(
            Modifier.fillMaxWidth().background(if (style == DesignStyle.Material) colors.bgSubtle else colors.toolbarBg).padding(6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            OcticonColored(Octicons.Repo, null, colors.toolbarText, size = 12.dp)
            Text(repository, style = MaterialTheme.typography.labelSmall, color = colors.toolbarText, maxLines = 1)
        }
        Box(Modifier.fillMaxWidth().height(2.dp).background(colors.tabBarActive))
        listOf(colors.fileModified, colors.fileNew).forEach { tint ->
            Row(Modifier.padding(horizontal = 6.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(8.dp).clip(RoundedCornerShape(2.dp)).background(colors.accent.emphasis))
                Box(Modifier.padding(start = 4.dp).size(8.dp).clip(RoundedCornerShape(2.dp)).background(tint))
                Box(
                    Modifier
                        .padding(start = 4.dp)
                        .height(4.dp)
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(2.dp))
                        .background(colors.borderDefault),
                )
            }
        }
        Box(Modifier.fillMaxWidth().weight(1f).padding(6.dp), contentAlignment = Alignment.BottomCenter) {
            val primary = when (style) {
                DesignStyle.GitHubMobile -> colors.success.emphasis
                DesignStyle.GitHubDesktop, DesignStyle.Material -> colors.accent.emphasis
            }
            val shape = if (style == DesignStyle.Material) RoundedCornerShape(50) else corner
            Text(
                button,
                Modifier.fillMaxWidth().clip(shape).background(primary).padding(vertical = 4.dp),
                style = MaterialTheme.typography.labelMedium,
                color = colors.textOnEmphasis,
                textAlign = TextAlign.Center,
                maxLines = 1,
            )
        }
    }
}

private const val MINIATURE_HEIGHT = 110
