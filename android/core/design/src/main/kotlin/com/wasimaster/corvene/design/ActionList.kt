package com.wasimaster.corvene.design

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Primer's ActionList: a column of [ActionListItem]s. GitHub Desktop draws
 * it as a bordered box (GHD's lists), the other styles flush with the screen.
 */
@Composable
fun ActionList(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val colors = CorveneTheme.colors
    val desktop = LocalDesignStyle.current == DesignStyle.GitHubDesktop
    val shape = RoundedCornerShape(CorveneTheme.metrics.cornerMedium)
    Column(
        modifier
            .fillMaxWidth()
            .then(if (desktop) Modifier.clip(shape).border(1.dp, colors.borderDefault, shape) else Modifier)
            .background(colors.bgDefault),
        content = content,
    )
}

/**
 * A row of a Primer ActionList: [leading] visual, [title] with an optional
 * [description] line, and trailing visuals: a [count], a [checked] mark
 * (single-select lists; `null` draws no check column), a [chevron] when more
 * follows, or anything in [trailing]. Long-press is the context menu's gesture.
 * [danger] colours the title for destructive items.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ActionListItem(
    title: String,
    modifier: Modifier = Modifier,
    description: String? = null,
    selected: Boolean = false,
    enabled: Boolean = true,
    danger: Boolean = false,
    count: Int? = null,
    checked: Boolean? = null,
    chevron: Boolean = false,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val metrics = CorveneTheme.metrics
    val colors = CorveneTheme.colors
    val clickable = if (onClick != null || onLongClick != null) {
        Modifier.combinedClickable(
            enabled = enabled,
            role = Role.Button,
            onLongClick = onLongClick,
            onClick = onClick ?: {},
        )
    } else {
        Modifier
    }
    Row(
        modifier
            .fillMaxWidth()
            .semantics { this.selected = selected || checked == true }
            .background(if (selected) colors.bgSelected else Color.Transparent)
            .then(clickable)
            .defaultMinSize(minHeight = if (description != null) metrics.rowHeightLarge else metrics.rowHeight)
            .padding(horizontal = metrics.gutter, vertical = CorveneTheme.spacing.s),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.m),
    ) {
        if (leading != null) leading()
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = when {
                    !enabled -> colors.textDisabled
                    danger -> colors.danger.fg
                    else -> colors.textPrimary
                },
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (description != null) {
                Text(
                    description,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.MiddleEllipsis,
                )
            }
        }
        if (trailing != null || count != null || checked != null || chevron) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.s),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                trailing?.invoke(this)
                if (count != null) {
                    Text(count.toString(), style = MaterialTheme.typography.bodyMedium, color = colors.textSecondary)
                }
                if (checked != null) {
                    // the column stays when unchecked, so labels line up
                    Box(Modifier.size(16.dp)) { if (checked) Octicon(Octicons.Check, null, tint = OcticonTint.Link) }
                }
                if (chevron) Octicon(Octicons.ChevronRight, null, tint = OcticonTint.Secondary)
            }
        }
    }
}

/** A group's heading inside an ActionList ("Recent", "GitHub.com", "Other"). */
@Composable
fun ActionListGroupHeader(title: String, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    val desktop = LocalDesignStyle.current == DesignStyle.GitHubDesktop
    Text(
        title,
        modifier = modifier
            .fillMaxWidth()
            .background(if (desktop) colors.bgInset else Color.Transparent)
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = if (desktop) 4.dp else 8.dp),
        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
        color = if (desktop) colors.textPrimary else colors.textSecondary,
    )
}

/** The rule between rows: inset past the leading visual in Mobile and Material, full width in Desktop. */
@Composable
fun ActionListDivider(modifier: Modifier = Modifier, leadingInset: Boolean = true) {
    val metrics = CorveneTheme.metrics
    val start = if (metrics.insetDividers && leadingInset) metrics.gutter + metrics.iconTile + CorveneTheme.spacing.m else 0.dp
    HorizontalDivider(modifier.padding(start = start), thickness = 1.dp, color = CorveneTheme.colors.borderMuted)
}

/** GitHub Mobile's coloured square behind a top-level row's icon; a plain icon in the other styles. */
@Composable
fun IconTile(icon: OcticonIcon, tint: SemanticColor, modifier: Modifier = Modifier) {
    val style = LocalDesignStyle.current
    val size = CorveneTheme.metrics.iconTile
    when (style) {
        DesignStyle.GitHubMobile -> Box(
            modifier.size(size).clip(RoundedCornerShape(CorveneTheme.metrics.cornerMedium)).background(tint.emphasis),
            contentAlignment = Alignment.Center,
        ) { OcticonColored(icon, null, CorveneTheme.colors.textOnEmphasis, size = 16.dp) }
        DesignStyle.Material -> Box(
            modifier.size(size).clip(RoundedCornerShape(50)).background(MaterialTheme.colorScheme.secondaryContainer),
            contentAlignment = Alignment.Center,
        ) { OcticonColored(icon, null, MaterialTheme.colorScheme.onSecondaryContainer, size = 24.dp) }
        DesignStyle.GitHubDesktop -> Box(modifier.size(size), contentAlignment = Alignment.Center) {
            OcticonColored(icon, null, CorveneTheme.colors.iconPrimary, size = 16.dp)
        }
    }
}

@Preview(widthDp = 360)
@Composable
private fun ActionListPreview() {
    DesignStyleSamples {
        ActionList {
            ActionListGroupHeader("Recent")
            ActionListItem(
                "corvene",
                description = "~/Work/corvene",
                selected = true,
                leading = { IconTile(Octicons.Repo, CorveneTheme.colors.accent) },
                trailing = { CounterLabel("3") },
            )
            ActionListDivider()
            ActionListItem("desktop", leading = { IconTile(Octicons.RepoForked, CorveneTheme.colors.done) }, count = 12)
            ActionListDivider()
            ActionListItem("main", checked = true, leading = { Octicon(Octicons.GitBranch, null) })
            ActionListItem("Appearance", chevron = true, leading = { Octicon(Octicons.Paintbrush, null) })
            ActionListItem("Remove", danger = true, leading = { Octicon(Octicons.Trash, null, tint = OcticonTint.Danger) })
        }
    }
}
