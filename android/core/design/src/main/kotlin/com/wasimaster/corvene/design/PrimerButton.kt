package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/** Primer's button variants. */
enum class PrimerButtonVariant {
    /** The view's main action: green in GitHub Mobile, blue in GitHub Desktop, filled in Material. */
    Primary,

    /** Everything else. */
    Default,

    /** Destroys something. */
    Danger,

    /** No chrome until pressed. */
    Invisible,

    /** Looks like a link. */
    Link,
}

/**
 * A button in the style's skin: Primer's for the two GitHub styles, the M3
 * button family for Material. [leadingIcon] is an Octicon drawn at 16 dp.
 */
@Composable
fun PrimerButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    variant: PrimerButtonVariant = PrimerButtonVariant.Default,
    enabled: Boolean = true,
    leadingIcon: OcticonIcon? = null,
) {
    if (LocalDesignStyle.current == DesignStyle.Material) {
        MaterialButton(text, onClick, modifier, variant, enabled, leadingIcon)
        return
    }
    val c = CorveneTheme.colors
    val desktop = LocalDesignStyle.current == DesignStyle.GitHubDesktop
    val (container, content, border) = when (variant) {
        PrimerButtonVariant.Primary ->
            Triple(if (desktop) c.accent.emphasis else c.success.emphasis, c.textOnEmphasis, Color.Transparent)
        PrimerButtonVariant.Default -> Triple(c.bgSubtle, c.textPrimary, c.borderDefault)
        PrimerButtonVariant.Danger -> Triple(c.bgSubtle, c.danger.fg, c.borderDefault)
        PrimerButtonVariant.Invisible -> Triple(Color.Transparent, c.textPrimary, Color.Transparent)
        PrimerButtonVariant.Link -> Triple(Color.Transparent, c.textLink, Color.Transparent)
    }
    val alpha = if (enabled) 1f else DISABLED_ALPHA
    Surface(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.minimumInteractiveComponentSize(),
        shape = RoundedCornerShape(CorveneTheme.metrics.cornerMedium),
        color = container.copy(alpha = container.alpha * alpha),
        contentColor = content.copy(alpha = alpha),
        border = if (border == Color.Transparent) null else BorderStroke(1.dp, border),
    ) {
        Row(
            Modifier
                .defaultMinSize(minHeight = CorveneTheme.metrics.controlHeight)
                .padding(horizontal = if (variant == PrimerButtonVariant.Link) 4.dp else CorveneTheme.spacing.m),
            horizontalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.s, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (leadingIcon != null) OcticonColored(leadingIcon, null, content.copy(alpha = alpha))
            Text(text, style = MaterialTheme.typography.labelLarge)
        }
    }
}

@Composable
private fun MaterialButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier,
    variant: PrimerButtonVariant,
    enabled: Boolean,
    leadingIcon: OcticonIcon?,
) {
    val padding = if (leadingIcon != null) ButtonDefaults.ButtonWithIconContentPadding else ButtonDefaults.ContentPadding
    val label: @Composable () -> Unit = {
        if (leadingIcon != null) {
            OcticonColored(leadingIcon, null, LocalContentColor.current, Modifier.padding(end = 8.dp))
        }
        Text(text)
    }
    when (variant) {
        PrimerButtonVariant.Primary -> Button(onClick, modifier, enabled, contentPadding = padding) { label() }
        PrimerButtonVariant.Default -> OutlinedButton(onClick, modifier, enabled, contentPadding = padding) { label() }
        PrimerButtonVariant.Danger -> TextButton(
            onClick,
            modifier,
            enabled,
            colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
            contentPadding = padding,
        ) { label() }
        PrimerButtonVariant.Invisible, PrimerButtonVariant.Link ->
            TextButton(onClick, modifier, enabled, contentPadding = PaddingValues(horizontal = 12.dp)) { label() }
    }
}

private const val DISABLED_ALPHA = 0.5f

@Preview(widthDp = 360)
@Composable
private fun PrimerButtonPreview() {
    DesignStyleSamples {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PrimerButton("Commit", {}, variant = PrimerButtonVariant.Primary, leadingIcon = Octicons.GitCommit)
            PrimerButton("Fetch", {}, leadingIcon = Octicons.Sync)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PrimerButton("Discard", {}, variant = PrimerButtonVariant.Danger)
            PrimerButton("Cancel", {}, variant = PrimerButtonVariant.Invisible)
            PrimerButton("Learn more", {}, variant = PrimerButtonVariant.Link)
        }
    }
}
