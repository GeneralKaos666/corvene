package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Primer's SegmentedControl: 2–5 text segments, applied at once (Unified /
 * Split). Material draws M3's segmented buttons. [fill] shares the width
 * equally (a control as wide as its row).
 */
@Composable
fun SegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    fill: Boolean = false,
) {
    val colors = CorveneTheme.colors
    if (LocalDesignStyle.current == DesignStyle.Material) {
        SingleChoiceSegmentedButtonRow(modifier) {
            options.forEachIndexed { index, label ->
                SegmentedButton(
                    selected = index == selectedIndex,
                    onClick = { onSelect(index) },
                    shape = SegmentedButtonDefaults.itemShape(index, options.size),
                    modifier = if (fill) Modifier.weight(1f) else Modifier,
                ) { Text(label, maxLines = 1) }
            }
        }
        return
    }
    val outer = RoundedCornerShape(CorveneTheme.metrics.cornerMedium)
    Row(
        modifier
            .height(CorveneTheme.metrics.controlHeight)
            .clip(outer)
            .background(colors.bgSubtle)
            .border(1.dp, colors.borderDefault, outer)
            .padding(2.dp)
            .selectableGroup(),
    ) {
        options.forEachIndexed { index, label ->
            val selected = index == selectedIndex
            val inner = RoundedCornerShape(CorveneTheme.metrics.cornerMedium - 1.dp)
            Box(
                Modifier
                    .then(if (fill) Modifier.weight(1f) else Modifier)
                    .fillMaxHeight()
                    .clip(inner)
                    .then(if (selected) Modifier.background(colors.bgDefault).border(1.dp, colors.borderDefault, inner) else Modifier)
                    .selectable(selected, role = Role.Tab, onClick = { onSelect(index) })
                    .padding(horizontal = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    label,
                    style = MaterialTheme.typography.labelLarge.copy(fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal),
                    color = colors.textPrimary,
                    maxLines = 1,
                )
            }
        }
    }
}

/** One tab of an [UnderlineNav]: a label, an optional count and icon. */
@Immutable
data class UnderlineNavItem(val label: String, val count: Int? = null, val icon: OcticonIcon? = null)

/**
 * Primer's UnderlineNav: tabs over the content they switch (Changes [3] |
 * History). GitHub Mobile draws full-width tabs with the orange underline,
 * GitHub Desktop its tab bar (equal tabs, a blue underline, a count pill),
 * Material M3's primary tab row.
 */
@Composable
fun UnderlineNav(items: List<UnderlineNavItem>, selectedIndex: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    val style = LocalDesignStyle.current
    if (style == DesignStyle.Material) {
        PrimaryTabRow(selectedTabIndex = selectedIndex, modifier = modifier, containerColor = colors.bgCanvas) {
            items.forEachIndexed { index, item ->
                Tab(
                    selected = index == selectedIndex,
                    onClick = { onSelect(index) },
                    text = { TabLabel(item, index == selectedIndex) },
                )
            }
        }
        return
    }
    val height = if (style == DesignStyle.GitHubDesktop) 40.dp else 48.dp
    Column(modifier.fillMaxWidth().background(if (style == DesignStyle.GitHubDesktop) colors.bgSubtle else colors.bgCanvas)) {
        Row(Modifier.fillMaxWidth().height(height).selectableGroup()) {
            items.forEachIndexed { index, item ->
                val selected = index == selectedIndex
                Box(
                    Modifier
                        .weight(1f)
                        .height(height)
                        .then(if (selected && style == DesignStyle.GitHubDesktop) Modifier.background(colors.bgDefault) else Modifier)
                        .selectable(selected, role = Role.Tab, onClick = { onSelect(index) })
                        .drawBehind {
                            if (selected) {
                                val stroke = 2.dp.toPx()
                                val y = size.height - stroke / 2
                                drawLine(colors.tabBarActive, Offset(0f, y), Offset(size.width, y), stroke)
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) { TabLabel(item, selected) }
            }
        }
        HorizontalDivider(color = colors.borderMuted)
    }
}

@Composable
private fun TabLabel(item: UnderlineNavItem, selected: Boolean) {
    val colors = CorveneTheme.colors
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        if (item.icon != null) Octicon(item.icon, null, tint = if (selected) OcticonTint.Primary else OcticonTint.Secondary)
        Text(
            item.label,
            style = MaterialTheme.typography.labelLarge.copy(fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium),
            color = if (selected) colors.textPrimary else colors.textSecondary,
            maxLines = 1,
        )
        if (item.count != null) CounterLabel(item.count.toString())
    }
}

/**
 * A filter chip (GitHub Mobile's pill row: "Included", "New", …): tapping
 * toggles [selected]. [count] shows how many items it matches.
 */
@Composable
fun PrimerChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: OcticonIcon? = null,
    dropdown: Boolean = false,
    enabled: Boolean = true,
) {
    val colors = CorveneTheme.colors
    val style = LocalDesignStyle.current
    if (style == DesignStyle.Material) {
        FilterChip(
            selected = selected,
            onClick = onClick,
            label = { Text(label) },
            modifier = modifier,
            enabled = enabled,
            leadingIcon = if (selected) {
                { Octicon(Octicons.Check, null, tint = OcticonTint.Primary) }
            } else {
                leadingIcon?.let { { Octicon(it, null) } }
            },
            trailingIcon = if (dropdown) {
                { Octicon(Octicons.TriangleDown, null) }
            } else {
                null
            },
            colors = FilterChipDefaults.filterChipColors(),
        )
        return
    }
    val shape = RoundedCornerShape(if (style == DesignStyle.GitHubDesktop) CorveneTheme.metrics.cornerSmall else 50.dp)
    Surface(
        onClick = onClick,
        enabled = enabled,
        selected = selected,
        modifier = modifier.height(32.dp),
        shape = shape,
        color = if (selected) colors.accent.subtle else colors.bgSubtle,
        contentColor = if (selected) colors.accent.fg else colors.textPrimary,
        border = BorderStroke(1.dp, if (selected) colors.accent.muted else colors.borderMuted),
    ) {
        Row(
            Modifier.padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when {
                selected -> OcticonColored(Octicons.Check, null, colors.accent.fg, size = 14.dp)
                leadingIcon != null -> OcticonColored(leadingIcon, null, colors.iconSecondary, size = 14.dp)
            }
            Text(label, style = MaterialTheme.typography.bodyMedium, maxLines = 1)
            if (dropdown) OcticonColored(Octicons.TriangleDown, null, colors.iconSecondary, size = 14.dp)
        }
    }
}

/** A horizontally scrolling row of [PrimerChip]s with the style's gutter. */
@Composable
fun ChipRow(modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    Row(
        modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(PaddingValues(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp)),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

@Preview(widthDp = 360, heightDp = 900)
@Composable
private fun NavigationPreview() {
    DesignStyleSamples {
        UnderlineNav(listOf(UnderlineNavItem("Changes", 3), UnderlineNavItem("History")), 0, {})
        SegmentedControl(listOf("Unified", "Split"), 0, {})
        ChipRow {
            PrimerChip("Included", selected = true, onClick = {})
            PrimerChip("New", selected = false, onClick = {})
            PrimerChip("Sort", selected = false, onClick = {}, dropdown = true)
        }
    }
}
