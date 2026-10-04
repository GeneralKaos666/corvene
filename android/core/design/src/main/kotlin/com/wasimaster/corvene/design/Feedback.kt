package com.wasimaster.corvene.design

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/** The tone of a [Flash]. */
enum class FlashVariant { Default, Success, Warning, Danger }

/**
 * Primer's Flash / Banner: one message about the view ("2 stashed changes",
 * "Couldn't load the diff"), an optional [title], up to one [action] and a
 * dismiss button when [onDismiss] is set. [flush] drops the rounded box for
 * banners that run edge to edge inside a list or a dialog.
 */
@Composable
fun Flash(
    text: String,
    modifier: Modifier = Modifier,
    variant: FlashVariant = FlashVariant.Default,
    title: String? = null,
    icon: OcticonIcon? = null,
    flush: Boolean = false,
    dismissDescription: String = "",
    onDismiss: (() -> Unit)? = null,
    action: (@Composable () -> Unit)? = null,
) {
    val colors = CorveneTheme.colors
    val hue = when (variant) {
        FlashVariant.Default -> colors.accent
        FlashVariant.Success -> colors.success
        FlashVariant.Warning -> colors.attention
        FlashVariant.Danger -> colors.danger
    }
    val glyph = icon ?: when (variant) {
        FlashVariant.Default -> Octicons.Info
        FlashVariant.Success -> Octicons.CheckCircle
        FlashVariant.Warning, FlashVariant.Danger -> Octicons.Alert
    }
    val shape = RoundedCornerShape(if (flush) 0.dp else CorveneTheme.metrics.cornerMedium)
    Row(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(hue.subtle)
            .then(if (flush) Modifier else Modifier.border(1.dp, hue.muted, shape))
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OcticonColored(glyph, null, hue.fg)
        Column(Modifier.weight(1f)) {
            if (title != null) {
                Text(title, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold), color = colors.textPrimary)
            }
            Text(text, style = MaterialTheme.typography.bodyMedium, color = colors.textPrimary)
        }
        action?.invoke()
        if (onDismiss != null) {
            PrimerIconButton(Octicons.X, dismissDescription, onDismiss, Modifier.size(32.dp), tint = OcticonTint.Secondary)
        }
    }
}

/** Primer's Spinner sizes: 16, 32, 64 dp. */
enum class SpinnerSize(val dp: Int) { Small(16), Medium(32), Large(64) }

/** An indeterminate spinner with an accessible [label] ("Loading the diff"). */
@Composable
fun Spinner(modifier: Modifier = Modifier, size: SpinnerSize = SpinnerSize.Medium, label: String? = null) {
    val described = if (label != null) Modifier.semantics { contentDescription = label } else Modifier
    CircularProgressIndicator(
        modifier.size(size.dp.dp).then(described),
        color = CorveneTheme.colors.accent.fg,
        strokeWidth = if (size == SpinnerSize.Small) 2.dp else 3.dp,
        trackColor = CorveneTheme.colors.borderMuted,
        strokeCap = StrokeCap.Round,
    )
}

/**
 * Primer's ProgressBar: [progress] in 0..1, or null for the indeterminate
 * line screens show while their first query runs.
 */
@Composable
fun ProgressBar(progress: Float?, modifier: Modifier = Modifier, height: Int = 4) {
    val colors = CorveneTheme.colors
    val shaped = modifier.fillMaxWidth().height(height.dp).clip(RoundedCornerShape(50))
    if (progress == null) {
        LinearProgressIndicator(shaped, color = colors.accent.emphasis, trackColor = colors.borderMuted)
    } else {
        LinearProgressIndicator(
            progress = { progress.coerceIn(0f, 1f) },
            modifier = shaped,
            color = colors.success.emphasis,
            trackColor = colors.borderMuted,
            gapSize = 0.dp,
            drawStopIndicator = {},
        )
    }
}

@Preview(widthDp = 360, heightDp = 1000)
@Composable
private fun FeedbackPreview() {
    DesignStyleSamples {
        Flash("2 stashed changes", icon = Octicons.Stack)
        Flash("Couldn't load the diff", variant = FlashVariant.Danger, title = "Error", onDismiss = {})
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Spinner(size = SpinnerSize.Small)
            Spinner()
            ProgressBar(0.4f, Modifier.weight(1f))
        }
        ProgressBar(null)
    }
}
