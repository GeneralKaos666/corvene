package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Row
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * An icon-only button with a 48 dp target. Bars draw their actions with
 * [OcticonTint.Link] (Primer Mobile: chrome icons are link blue; the tint
 * defaults to [LocalOcticonTint], which bars set); the GitHub
 * Desktop style keeps them in the text colour like GHD's toolbar.
 */
@Composable
fun PrimerIconButton(
    icon: OcticonIcon,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: OcticonTint = LocalOcticonTint.current,
    enabled: Boolean = true,
) {
    val desktop = LocalDesignStyle.current == DesignStyle.GitHubDesktop
    IconButton(onClick = onClick, modifier = modifier, enabled = enabled) {
        val size = if (desktop) 16.dp else 24.dp
        if (desktop && tint == OcticonTint.Link) {
            // GHD's toolbar icons take the bar's text colour
            OcticonColored(icon, contentDescription, LocalContentColor.current, size = size)
        } else {
            Octicon(icon, contentDescription, tint = tint, size = size)
        }
    }
}

@Preview(widthDp = 240)
@Composable
private fun PrimerIconButtonPreview() {
    DesignStyleSamples {
        Row {
            PrimerIconButton(Octicons.Search, "Search", {}, tint = OcticonTint.Link)
            PrimerIconButton(Octicons.Plus, "Add", {}, tint = OcticonTint.Link)
            PrimerIconButton(Octicons.KebabHorizontal, "More", {})
            PrimerIconButton(Octicons.Trash, "Remove", {}, tint = OcticonTint.Danger)
        }
    }
}
