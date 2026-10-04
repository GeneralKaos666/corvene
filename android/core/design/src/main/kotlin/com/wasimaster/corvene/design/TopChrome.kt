package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.material3.LocalContentColor
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
import androidx.compose.ui.platform.testTag
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
 * Current branch ▾ | push/pull) on the toolbar colour, then [tabs]; compact
 * widths drop the toolbar's captions and keep the values.
 * [repositoryAnchor], [branchAnchor] and [syncAnchor] draw inside the
 * triggers' boxes, where anchored pickers and menus belong. A long press on
 * the sync button runs [onSyncLongClick] (GHD's push/pull dropdown).
 *
 * Short windows (height < 480 dp, a phone in landscape) fold the branch row
 * into the app bar, so the content below keeps its rows.
 */
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
    onSyncLongClick: (() -> Unit)? = null,
    repositoryAnchor: @Composable () -> Unit = {},
    branchAnchor: @Composable () -> Unit = {},
    syncAnchor: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    tabs: @Composable () -> Unit = {},
) {
    val slots = ChromeSlots(repositoryAnchor, branchAnchor, syncAnchor, actions)
    when {
        LocalDesignStyle.current == DesignStyle.GitHubDesktop -> Column(modifier.fillMaxWidth()) {
            DesktopToolbar(repository, branch, sync, onRepositoryClick, onBranchClick, onSync, onSyncLongClick, labels, onBack, slots)
            tabs()
        }
        isShortHeight() -> Column(modifier.fillMaxWidth().background(chromeBackground())) {
            ShortBar(repository, branch, sync, onRepositoryClick, onBranchClick, onSync, onSyncLongClick, labels, onBack, slots)
            tabs()
        }
        else -> Column(modifier.fillMaxWidth().background(chromeBackground())) {
            MobileBar(repository, owner, onRepositoryClick, labels, onBack, slots)
            Row(
                Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter).padding(bottom = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // the chip is measured first (up to its cap); the sync button takes what is left and truncates
                Box {
                    BranchChip(branch ?: labels.noBranch, onBranchClick, labels.switchBranch)
                    branchAnchor()
                }
                Box(Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
                    SyncButton(sync, onSync, onSyncLongClick)
                    syncAnchor()
                }
            }
            tabs()
        }
    }
}

/** The anchors and actions the three layouts place. */
private class ChromeSlots(
    val repositoryAnchor: @Composable () -> Unit,
    val branchAnchor: @Composable () -> Unit,
    val syncAnchor: @Composable () -> Unit,
    val actions: @Composable RowScope.() -> Unit,
)

@Composable
private fun chromeBackground() =
    if (LocalDesignStyle.current == DesignStyle.Material) MaterialTheme.colorScheme.surface else CorveneTheme.colors.toolbarBg

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MobileBar(
    repository: String,
    owner: String?,
    onRepositoryClick: () -> Unit,
    labels: RepositoryChromeLabels,
    onBack: (() -> Unit)?,
    slots: ChromeSlots,
) {
    CompositionLocalProvider(LocalOcticonTint provides OcticonTint.Link) {
        TopAppBar(
            title = { RepositorySwitcher(repository, owner, onRepositoryClick, labels, slots.repositoryAnchor) },
            navigationIcon = {
                if (onBack != null) PrimerIconButton(Octicons.ArrowLeft, labels.back, onBack, tint = OcticonTint.Link)
            },
            actions = slots.actions,
            colors = barColors(),
        )
    }
}

/** One 48 dp row: back, repository ▾, branch chip, sync, actions. */
@Composable
private fun ShortBar(
    repository: String,
    branch: String?,
    sync: SyncButtonModel,
    onRepositoryClick: () -> Unit,
    onBranchClick: () -> Unit,
    onSync: () -> Unit,
    onSyncLongClick: (() -> Unit)?,
    labels: RepositoryChromeLabels,
    onBack: (() -> Unit)?,
    slots: ChromeSlots,
) {
    CompositionLocalProvider(
        LocalOcticonTint provides OcticonTint.Link,
        LocalContentColor provides CorveneTheme.colors.toolbarText,
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
                .height(SHORT_BAR_HEIGHT.dp)
                .padding(end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (onBack != null) {
                PrimerIconButton(Octicons.ArrowLeft, labels.back, onBack, tint = OcticonTint.Link)
            } else {
                Box(Modifier.width(8.dp))
            }
            Box(Modifier.weight(1f, fill = false)) {
                RepositorySwitcher(repository, null, onRepositoryClick, labels, slots.repositoryAnchor)
            }
            Box {
                BranchChip(branch ?: labels.noBranch, onBranchClick, labels.switchBranch)
                slots.branchAnchor()
            }
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
                SyncButton(sync, onSync, onSyncLongClick)
                slots.syncAnchor()
            }
            Row(content = slots.actions)
        }
    }
}

@Composable
private fun RepositorySwitcher(
    repository: String,
    owner: String?,
    onClick: () -> Unit,
    labels: RepositoryChromeLabels,
    anchor: @Composable () -> Unit,
) {
    Box {
        Row(
            Modifier.clickable(role = Role.Button, onClickLabel = labels.switchRepository, onClick = onClick),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            RepositoryTitle(owner, repository, Modifier.weight(1f, fill = false))
            Octicon(Octicons.ChevronDown, null, tint = OcticonTint.Secondary)
        }
        anchor()
    }
}

@Composable
private fun DesktopToolbar(
    repository: String,
    branch: String?,
    sync: SyncButtonModel,
    onRepositoryClick: () -> Unit,
    onBranchClick: () -> Unit,
    onSync: () -> Unit,
    onSyncLongClick: (() -> Unit)?,
    labels: RepositoryChromeLabels,
    onBack: (() -> Unit)?,
    slots: ChromeSlots,
) {
    val colors = CorveneTheme.colors
    // GHD's captions ("Current repository") cost a line a phone cannot spare
    val captions = !isCompactWidth()
    Row(
        Modifier
            .fillMaxWidth()
            .background(colors.toolbarBg)
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
            .height(if (captions) TOOLBAR_HEIGHT.dp else TOOLBAR_HEIGHT_COMPACT.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) {
            PrimerIconButton(Octicons.ArrowLeft, labels.back, onBack, tint = OcticonTint.Primary)
        }
        ToolbarButton(
            ToolbarButtonSpec(Octicons.Repo, labels.currentRepository.takeIf { captions }, repository, dropdown = captions),
            onRepositoryClick,
            Modifier.weight(1f),
            anchor = slots.repositoryAnchor,
        )
        ToolbarDivider()
        ToolbarButton(
            ToolbarButtonSpec(Octicons.GitBranch, labels.currentBranch.takeIf { captions }, branch ?: labels.noBranch, dropdown = captions),
            onBranchClick,
            Modifier.weight(1f),
            anchor = slots.branchAnchor,
        )
        ToolbarDivider()
        ToolbarButton(
            if (captions) {
                ToolbarButtonSpec(sync.icon, sync.title, sync.description.orEmpty(), dropdown = false, progress = sync.progress)
            } else {
                ToolbarButtonSpec(sync.icon, null, sync.title, dropdown = false, progress = sync.progress)
            },
            onSync,
            // on a phone the push/pull button gives way first, so the branch name stays readable
            Modifier.weight(if (captions) 1f else SYNC_WEIGHT_COMPACT),
            anchor = slots.syncAnchor,
            enabled = sync.enabled,
            onLongClick = onSyncLongClick,
        )
        CompositionLocalProvider(LocalOcticonTint provides OcticonTint.Primary) { Row(content = slots.actions) }
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
        modifier = Modifier.height(36.dp).testTag(TAG_BRANCH_CHIP),
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

/**
 * The push/pull button of the Mobile and Material chrome: a Default button
 * that truncates its label before the branch chip gives way, takes a long
 * press, and fills a progress line along its bottom while git runs.
 */
@Composable
private fun SyncButton(sync: SyncButtonModel, onClick: () -> Unit, onLongClick: (() -> Unit)?) {
    val colors = CorveneTheme.colors
    val material = LocalDesignStyle.current == DesignStyle.Material
    val shape = if (material) RoundedCornerShape(50) else RoundedCornerShape(CorveneTheme.metrics.cornerMedium)
    val content = if (sync.enabled) colors.textPrimary else colors.textDisabled
    Box(
        Modifier
            .height(36.dp)
            .clip(shape)
            .background(if (material) MaterialTheme.colorScheme.surfaceContainerHigh else colors.bgSubtle)
            .border(1.dp, if (material) MaterialTheme.colorScheme.outlineVariant else colors.borderDefault, shape)
            .combinedClickable(
                enabled = sync.enabled || onLongClick != null,
                role = Role.Button,
                onLongClick = onLongClick,
                onClick = { if (sync.enabled) onClick() },
            )
            .testTag(TAG_SYNC),
    ) {
        Row(
            Modifier.fillMaxHeight().padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OcticonColored(sync.icon, null, content)
            Text(sync.title, style = MaterialTheme.typography.labelLarge, color = content, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        val progress = sync.progress
        if (progress != null) {
            Box(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth(progress.coerceIn(0f, 1f))
                    .height(3.dp)
                    .background(colors.success.emphasis),
            )
        }
    }
}

/** What one GHD toolbar button shows: [label] is the caption line (none on compact widths). */
private class ToolbarButtonSpec(
    val icon: OcticonIcon,
    val label: String?,
    val value: String,
    val dropdown: Boolean,
    val progress: Float? = null,
)

@Composable
private fun ToolbarButton(
    spec: ToolbarButtonSpec,
    onClick: () -> Unit,
    modifier: Modifier,
    anchor: @Composable () -> Unit = {},
    enabled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
) {
    val colors = CorveneTheme.colors
    Box(modifier.fillMaxHeight()) {
        val progress = spec.progress
        if (progress != null) {
            // GHD fills the push/pull button behind its label
            Box(Modifier.fillMaxHeight().fillMaxWidth(progress.coerceIn(0f, 1f)).background(colors.toolbarButtonHoverBg))
        }
        Row(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth()
                .combinedClickable(enabled = enabled || onLongClick != null, role = Role.Button, onLongClick = onLongClick) {
                    if (enabled) onClick()
                }
                .padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OcticonColored(spec.icon, null, if (enabled) colors.toolbarText else colors.toolbarTextSecondary)
            Column(Modifier.weight(1f)) {
                if (spec.label != null) {
                    Text(spec.label, style = MaterialTheme.typography.labelSmall, color = colors.toolbarTextSecondary, maxLines = 1)
                }
                Text(
                    spec.value,
                    style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = if (enabled) colors.toolbarText else colors.toolbarTextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (spec.dropdown) OcticonColored(Octicons.TriangleDown, null, colors.toolbarTextSecondary)
        }
        anchor()
    }
}

@Composable
private fun ToolbarDivider() {
    VerticalDivider(Modifier.fillMaxHeight().width(1.dp), color = CorveneTheme.colors.toolbarButtonBorder)
}

/** Test tags of the chrome's triggers. */
const val TAG_BRANCH_CHIP = "cvd_branch_chip"
const val TAG_SYNC = "cvd_sync"

private const val TOOLBAR_HEIGHT = 50
private const val SYNC_WEIGHT_COMPACT = 0.8f
private const val TOOLBAR_HEIGHT_COMPACT = 44
private const val SHORT_BAR_HEIGHT = 48
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
