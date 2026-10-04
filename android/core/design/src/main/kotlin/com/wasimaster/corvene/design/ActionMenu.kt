package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Primer's ActionMenu: an [ActionList] of quick actions in an overlay,
 * anchored to its parent (place it in the same Box as the trigger). Items are
 * [ActionMenuItem]s; form controls belong in a [SelectPanel] or a dialog,
 * except the few view options a gear menu toggles in place.
 */
@Composable
fun ActionMenu(
    expanded: Boolean,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = CorveneTheme.colors
    val material = LocalDesignStyle.current == DesignStyle.Material
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        shape = if (material) MenuDefaults.shape else RoundedCornerShape(CorveneTheme.metrics.cornerLarge),
        containerColor = if (material) MenuDefaults.containerColor else colors.bgOverlay,
        border = if (material) null else BorderStroke(1.dp, colors.borderDefault),
        content = content,
    )
}

/**
 * One action of an [ActionMenu]: [text] with an optional leading Octicon,
 * a [checked] mark for the selected option of a single-select group (`null`:
 * not selectable), red when [danger], and an optional [trailing] control.
 */
@Composable
fun ActionMenuItem(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: OcticonIcon? = null,
    checked: Boolean? = null,
    danger: Boolean = false,
    enabled: Boolean = true,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val colors = CorveneTheme.colors
    val color = if (danger) colors.danger.fg else colors.textPrimary
    DropdownMenuItem(
        text = { Text(text, style = MaterialTheme.typography.bodyMedium) },
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        leadingIcon = when {
            checked != null -> {
                { Box(Modifier.size(16.dp)) { if (checked) Octicon(Octicons.Check, null, tint = OcticonTint.Link) } }
            }
            leadingIcon != null -> {
                { OcticonColored(leadingIcon, null, if (danger) colors.danger.fg else colors.iconPrimary) }
            }
            else -> null
        },
        trailingIcon = trailing?.let { slot -> { Row(content = slot) } },
        colors = MenuDefaults.itemColors(
            textColor = color,
            disabledTextColor = colors.textDisabled,
        ),
    )
}

/** A rule between groups of an [ActionMenu]. */
@Composable
fun ActionMenuDivider(modifier: Modifier = Modifier) {
    HorizontalDivider(modifier.padding(vertical = 4.dp), color = CorveneTheme.colors.borderMuted)
}

/** A heading over a group of an [ActionMenu] ("View", "Whitespace"). */
@Composable
fun ActionMenuGroupHeader(title: String, modifier: Modifier = Modifier) {
    Text(
        title,
        modifier.padding(horizontal = 16.dp, vertical = 6.dp),
        style = MaterialTheme.typography.labelMedium,
        color = CorveneTheme.colors.textSecondary,
    )
}

@Preview(widthDp = 280, heightDp = 900)
@Composable
private fun ActionMenuPreview() {
    DesignStyleSamples {
        // the menu's content without its popup window
        Column(
            Modifier
                .width(240.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(CorveneTheme.colors.bgOverlay),
        ) {
            ActionMenuGroupHeader("View")
            ActionMenuItem("Unified", {}, checked = true)
            ActionMenuItem("Split", {}, checked = false)
            ActionMenuDivider()
            ActionMenuItem("Copy path", {}, leadingIcon = Octicons.Copy)
            ActionMenuItem("Discard changes", {}, leadingIcon = Octicons.Trash, danger = true)
        }
    }
}
