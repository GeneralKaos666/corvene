package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties

/** True below 600 dp of width: one pane, sheets instead of popups. */
@Composable
fun isCompactWidth(): Boolean = LocalConfiguration.current.screenWidthDp < COMPACT_MAX_DP

private const val COMPACT_MAX_DP = 600

/**
 * True below 480 dp of height (a phone in landscape): chrome folds into one
 * bar and secondary rows hide, so lists keep visible rows.
 */
@Composable
fun isShortHeight(): Boolean = LocalConfiguration.current.screenHeightDp < SHORT_MAX_DP

private const val SHORT_MAX_DP = 480

/**
 * Primer's SelectPanel: pick from a filterable, grouped list (the repository
 * picker, the branch picker). A modal bottom sheet on compact widths, a popup
 * anchored under its parent on wider ones (place it in the trigger's Box).
 * The header has the [title] and a close button, then the [FilterField] when
 * [onFilterChange] is set, the [items] ([ActionListGroupHeader] and
 * [ActionListItem] with `checked`), and an optional [footer] (links or
 * buttons). Single-select panels close themselves from the item's onClick.
 * [titleActions] sit beside the title (GHD's "New Branch"), [tabs] under it
 * (the branch picker's Branches | Pull Requests). [inline] draws the panel in
 * place, without a sheet or popup (a destination of its own, screenshots).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SelectPanel(
    title: String,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    filter: String = "",
    onFilterChange: ((String) -> Unit)? = null,
    filterPlaceholder: String = "",
    closeDescription: String = "",
    footer: (@Composable RowScope.() -> Unit)? = null,
    titleActions: (@Composable RowScope.() -> Unit)? = null,
    tabs: (@Composable () -> Unit)? = null,
    inline: Boolean = false,
    items: LazyListScope.() -> Unit,
) {
    val colors = CorveneTheme.colors
    val parts = SelectPanelParts(
        title,
        onDismissRequest,
        filter,
        onFilterChange,
        filterPlaceholder,
        closeDescription,
        footer,
        titleActions,
        tabs,
    )
    if (inline) {
        Surface(modifier, color = colors.bgOverlay) { SelectPanelContent(parts, items) }
    } else if (isCompactWidth()) {
        ModalBottomSheet(
            onDismissRequest = onDismissRequest,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = colors.bgOverlay,
            modifier = modifier,
        ) {
            SelectPanelContent(parts, items)
        }
    } else {
        Popup(
            popupPositionProvider = BelowAnchor,
            onDismissRequest = onDismissRequest,
            properties = PopupProperties(focusable = true),
        ) {
            Surface(
                modifier = modifier.width(PANEL_WIDTH.dp).heightIn(max = PANEL_MAX_HEIGHT.dp),
                shape = RoundedCornerShape(CorveneTheme.metrics.cornerLarge),
                color = colors.bgOverlay,
                border = BorderStroke(1.dp, colors.borderDefault),
                shadowElevation = 8.dp,
            ) {
                SelectPanelContent(parts, items)
            }
        }
    }
}

/** What [SelectPanel] draws around its items. */
internal class SelectPanelParts(
    val title: String,
    val onDismissRequest: () -> Unit,
    val filter: String,
    val onFilterChange: ((String) -> Unit)?,
    val filterPlaceholder: String,
    val closeDescription: String,
    val footer: (@Composable RowScope.() -> Unit)?,
    val titleActions: (@Composable RowScope.() -> Unit)? = null,
    val tabs: (@Composable () -> Unit)? = null,
)

/** The panel without its sheet or popup (previews and screenshot tests draw this). */
@Composable
internal fun SelectPanelContent(parts: SelectPanelParts, items: LazyListScope.() -> Unit) {
    val colors = CorveneTheme.colors
    val title = parts.title
    val onDismissRequest = parts.onDismissRequest
    val filter = parts.filter
    val onFilterChange = parts.onFilterChange
    val filterPlaceholder = parts.filterPlaceholder
    val closeDescription = parts.closeDescription
    val footer = parts.footer
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().padding(start = CorveneTheme.metrics.gutter, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                title,
                Modifier.weight(1f),
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                color = colors.textPrimary,
            )
            parts.titleActions?.invoke(this)
            PrimerIconButton(Octicons.X, closeDescription, onDismissRequest, tint = OcticonTint.Secondary)
        }
        parts.tabs?.invoke()
        if (onFilterChange != null) {
            FilterField(
                filter,
                onFilterChange,
                Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 4.dp),
                placeholder = filterPlaceholder,
                clearDescription = closeDescription,
            )
        }
        LazyColumn(Modifier.fillMaxWidth().weight(1f, fill = false).heightIn(max = LIST_MAX_HEIGHT.dp), content = items)
        if (footer != null) {
            HorizontalDivider(color = colors.borderMuted)
            Row(
                Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
                content = footer,
            )
        }
    }
}

/** Under the anchor, start-aligned, kept inside the window. */
private object BelowAnchor : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset {
        val x = anchorBounds.left.coerceAtMost(windowSize.width - popupContentSize.width).coerceAtLeast(0)
        val below = anchorBounds.bottom
        val fits = below + popupContentSize.height <= windowSize.height
        val y = if (fits) below else (anchorBounds.top - popupContentSize.height).coerceAtLeast(0)
        return IntOffset(x, y)
    }
}

private const val PANEL_WIDTH = 360
private const val PANEL_MAX_HEIGHT = 520
private const val LIST_MAX_HEIGHT = 560

@Preview(widthDp = 360, heightDp = 1300)
@Composable
private fun SelectPanelPreview() {
    DesignStyleSamples {
        SelectPanelContent(
            SelectPanelParts(
                title = "Repositories",
                onDismissRequest = {},
                filter = "",
                onFilterChange = {},
                filterPlaceholder = "Filter",
                closeDescription = "Close",
                footer = { PrimerButton("Add repository", {}, leadingIcon = Octicons.Plus) },
            ),
        ) {
            item { ActionListGroupHeader("Recent") }
            item { ActionListItem("corvene", checked = true, leading = { Octicon(Octicons.Repo, null) }) }
            item { ActionListItem("desktop", checked = false, leading = { Octicon(Octicons.Repo, null) }) }
        }
    }
}
