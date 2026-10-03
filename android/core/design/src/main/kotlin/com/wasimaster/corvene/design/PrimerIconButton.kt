package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Row
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * An icon-only button with a 48 dp target. Bars draw their actions with
 * [OcticonTint.Link] (Primer Mobile: chrome icons are link blue); the GitHub
 * Desktop style keeps them in the text colour like GHD's toolbar.
 */
@Composable
fun PrimerIconButton(
    icon: OcticonIcon,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: OcticonTint = OcticonTint.Primary,
    enabled: Boolean = true,
) {
    val resolved = if (LocalDesignStyle.current == DesignStyle.GitHubDesktop && tint == OcticonTint.Link) OcticonTint.Primary else tint
    IconButton(onClick = onClick, modifier = modifier, enabled = enabled) {
        val size = if (LocalDesignStyle.current == DesignStyle.GitHubDesktop) 16.dp else 24.dp
        Octicon(icon, contentDescription, tint = resolved, size = size)
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
