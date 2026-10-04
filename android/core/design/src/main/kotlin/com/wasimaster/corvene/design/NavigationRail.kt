package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.NavigationRailItemDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/** One destination of a [RepositoryRail]: its label, icon, a count badge, whether it is a place or an action (a sheet). */
@Immutable
data class RailItem(val label: String, val icon: OcticonIcon, val count: Int? = null, val tag: String = "")

/**
 * Whether the repository screen navigates with a [RepositoryRail] instead of
 * the tabs under the chrome: GitHub Mobile and Material on medium and
 * expanded widths (M3's rule). GitHub Desktop keeps GHD's tab bar over the
 * sidebar at every width.
 */
@Composable
fun usesNavigationRail(): Boolean = LocalDesignStyle.current != DesignStyle.GitHubDesktop && !isCompactWidth()

/**
 * Whether the window is expanded (≥ 840 dp), where the repository's list
 * pane is GHD's 250 dp sidebar instead of a share of the width.
 */
@Composable
fun isExpandedWidth(): Boolean = LocalConfiguration.current.screenWidthDp >= EXPANDED_MIN_DP

/**
 * M3's NavigationRail with Primer's colours (Mobile) or the dynamic scheme's
 * (Material): the repository's destinations (Changes [n], History) and the
 * pickers (Branches, Repositories), selected one in the indicator.
 */
@Composable
fun RepositoryRail(
    items: List<RailItem>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    header: (@Composable () -> Unit)? = null,
) {
    val colors = CorveneTheme.colors
    val material = LocalDesignStyle.current == DesignStyle.Material
    NavigationRail(
        modifier = modifier,
        containerColor = if (material) MaterialTheme.colorScheme.surface else colors.bgCanvas,
        header = header?.let { content -> { Column { content() } } },
        windowInsets = WindowInsets(0.dp),
    ) {
        Spacer(Modifier.height(8.dp))
        items.forEachIndexed { index, item ->
            val selected = index == selectedIndex
            NavigationRailItem(
                selected = selected,
                onClick = { onSelect(index) },
                modifier = if (item.tag.isNotEmpty()) Modifier.testTag(item.tag) else Modifier,
                icon = {
                    BadgedBox(
                        badge = {
                            if (item.count != null && item.count > 0) {
                                Badge(
                                    containerColor = if (material) MaterialTheme.colorScheme.primary else colors.accent.emphasis,
                                    contentColor = if (material) MaterialTheme.colorScheme.onPrimary else colors.textOnEmphasis,
                                ) { Text(item.count.toString()) }
                            }
                        },
                    ) {
                        OcticonColored(item.icon, null, if (selected) colors.textPrimary else colors.iconSecondary, size = 24.dp)
                    }
                },
                label = { Text(item.label, maxLines = 1) },
                colors = if (material) {
                    NavigationRailItemDefaults.colors()
                } else {
                    NavigationRailItemDefaults.colors(
                        selectedTextColor = colors.textPrimary,
                        unselectedTextColor = colors.textSecondary,
                        indicatorColor = colors.bgSelected,
                    )
                },
            )
        }
    }
}

private const val EXPANDED_MIN_DP = 840

@Preview(widthDp = 120, heightDp = 1200)
@Composable
private fun RepositoryRailPreview() {
    DesignStyleSamples {
        RepositoryRail(
            listOf(
                RailItem("Changes", Octicons.Diff, 3),
                RailItem("History", Octicons.History),
                RailItem("Branches", Octicons.GitBranch),
            ),
            selectedIndex = 0,
            onSelect = {},
        )
    }
}
