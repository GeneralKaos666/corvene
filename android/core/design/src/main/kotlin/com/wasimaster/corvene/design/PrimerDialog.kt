package com.wasimaster.corvene.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

/**
 * Primer's Dialog: a header with the [title] and a close button, a
 * scrolling body ([content]), and a footer with the [dismissButton] then the
 * [confirmButton] at the end. [fullScreenOnCompact] takes the whole screen on
 * compact widths (forms, Primer's narrow-viewport rule), with the confirm
 * button in the header. Tapping outside dismisses unless [dismissOnOutside]
 * is false (unsaved input). A dialog that is not [dismissible] (progress)
 * has no close button and ignores Back.
 */
@Composable
fun PrimerDialog(
    title: String,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    fullScreenOnCompact: Boolean = false,
    dismissOnOutside: Boolean = true,
    dismissible: Boolean = true,
    closeDescription: String = "",
    confirmButton: (@Composable () -> Unit)? = null,
    dismissButton: (@Composable () -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val fullScreen = fullScreenOnCompact && isCompactWidth()
    Dialog(
        onDismissRequest = onDismissRequest,
        properties = DialogProperties(
            dismissOnBackPress = dismissible,
            dismissOnClickOutside = dismissOnOutside && dismissible,
            usePlatformDefaultWidth = !fullScreen,
            decorFitsSystemWindows = !fullScreen,
        ),
    ) {
        PrimerDialogSurface(
            title,
            onDismissRequest.takeIf { dismissible },
            modifier,
            fullScreen,
            closeDescription,
            confirmButton,
            dismissButton,
            content,
        )
    }
}

/** The dialog without its window (previews and screenshot tests draw this). */
@Composable
internal fun PrimerDialogSurface(
    title: String,
    onDismissRequest: (() -> Unit)?,
    modifier: Modifier,
    fullScreen: Boolean,
    closeDescription: String,
    confirmButton: (@Composable () -> Unit)?,
    dismissButton: (@Composable () -> Unit)?,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = CorveneTheme.colors
    val material = LocalDesignStyle.current == DesignStyle.Material
    val shape = when {
        fullScreen -> RoundedCornerShape(0.dp)
        material -> MaterialTheme.shapes.extraLarge
        else -> RoundedCornerShape(CorveneTheme.metrics.cornerLarge)
    }
    Surface(
        modifier = if (fullScreen) modifier.fillMaxSize() else modifier.widthIn(min = 280.dp, max = 480.dp),
        shape = shape,
        color = if (material) MaterialTheme.colorScheme.surfaceContainerHigh else colors.bgOverlay,
        border = if (material || fullScreen) null else BorderStroke(1.dp, colors.borderDefault),
        shadowElevation = if (fullScreen) 0.dp else 8.dp,
    ) {
        Column(if (fullScreen) Modifier.windowInsetsPadding(WindowInsets.safeDrawing).imePadding() else Modifier) {
            Row(
                Modifier.fillMaxWidth().padding(start = if (fullScreen) 4.dp else 16.dp, end = 4.dp, top = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (fullScreen && onDismissRequest != null) {
                    PrimerIconButton(Octicons.X, closeDescription, onDismissRequest, tint = OcticonTint.Link)
                }
                Text(
                    title,
                    Modifier.weight(1f).padding(vertical = 12.dp),
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = colors.textPrimary,
                )
                if (fullScreen) {
                    confirmButton?.invoke()
                } else if (!material && onDismissRequest != null) {
                    PrimerIconButton(Octicons.X, closeDescription, onDismissRequest, tint = OcticonTint.Secondary)
                }
            }
            Column(
                Modifier
                    .weight(1f, fill = fullScreen)
                    .heightIn(max = if (fullScreen) BODY_MAX_FULL.dp else BODY_MAX.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                content = content,
            )
            if (!fullScreen && (confirmButton != null || dismissButton != null)) {
                if (!material) HorizontalDivider(color = colors.borderMuted)
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    dismissButton?.invoke()
                    confirmButton?.invoke()
                }
            }
        }
    }
}

private const val BODY_MAX = 420
private const val BODY_MAX_FULL = 4000

@Preview(widthDp = 360, heightDp = 1000)
@Composable
private fun PrimerDialogPreview() {
    DesignStyleSamples {
        PrimerDialogSurface(
            title = "Discard changes?",
            onDismissRequest = {},
            modifier = Modifier,
            fullScreen = false,
            closeDescription = "Close",
            confirmButton = { PrimerButton("Discard", {}, variant = PrimerButtonVariant.Danger) },
            dismissButton = { PrimerButton("Cancel", {}) },
        ) {
            Text("README.md will be moved to the trash.", color = CorveneTheme.colors.textPrimary)
        }
    }
}
