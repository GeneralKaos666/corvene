package com.wasimaster.corvene.settings

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfo
import androidx.compose.material3.adaptive.layout.AnimatedPane
import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffold
import androidx.compose.material3.adaptive.layout.PaneAdaptedValue
import androidx.compose.material3.adaptive.layout.ThreePaneScaffoldValue
import androidx.compose.material3.adaptive.layout.calculatePaneScaffoldDirectiveWithTwoPanesOnMediumWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Settings on medium and expanded widths: the sections list as the list
 * pane (GHD's vertical nav, here wide enough for a tile and a word) and the
 * [selected] section beside it. Compact widths push the section as its own
 * destination instead (the app's back stack).
 */
@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
fun SettingsPanes(
    selected: SettingsSection,
    onSelect: (SettingsSection) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
    detail: @Composable (SettingsSection) -> Unit,
) {
    val directive = calculatePaneScaffoldDirectiveWithTwoPanesOnMediumWidth(currentWindowAdaptiveInfo())
    ListDetailPaneScaffold(
        directive = directive,
        value = BothPanes,
        modifier = modifier,
        listPane = {
            AnimatedPane(Modifier.preferredWidth(LIST_WIDTH.dp)) {
                SettingsListScreen(onOpen = onSelect, selected = selected, contentPadding = contentPadding)
            }
        },
        detailPane = { AnimatedPane { detail(selected) } },
    )
}

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
private val BothPanes = ThreePaneScaffoldValue(
    primary = PaneAdaptedValue.Expanded,
    secondary = PaneAdaptedValue.Expanded,
    tertiary = PaneAdaptedValue.Hidden,
)

private const val LIST_WIDTH = 220
