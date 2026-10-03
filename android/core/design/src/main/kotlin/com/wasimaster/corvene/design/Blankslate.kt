package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Primer's Blankslate: what an empty view says, and the one or two things to
 * do about it ([primaryAction], [secondaryAction]).
 */
@Composable
fun Blankslate(
    icon: OcticonIcon,
    title: String,
    modifier: Modifier = Modifier,
    description: String? = null,
    primaryAction: (@Composable () -> Unit)? = null,
    secondaryAction: (@Composable () -> Unit)? = null,
) {
    val colors = CorveneTheme.colors
    Column(
        modifier.fillMaxWidth().padding(horizontal = CorveneTheme.spacing.l, vertical = CorveneTheme.spacing.xl),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(CorveneTheme.spacing.s),
    ) {
        Octicon(icon, null, tint = OcticonTint.Secondary, size = 24.dp, modifier = Modifier.padding(bottom = 4.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, color = colors.textPrimary, textAlign = TextAlign.Center)
        if (description != null) {
            Text(description, style = MaterialTheme.typography.bodyMedium, color = colors.textSecondary, textAlign = TextAlign.Center)
        }
        if (primaryAction != null) {
            Column(Modifier.padding(top = CorveneTheme.spacing.s)) { primaryAction() }
        }
        if (secondaryAction != null) secondaryAction()
    }
}

@Preview(widthDp = 360)
@Composable
private fun BlankslatePreview() {
    DesignStyleSamples {
        Blankslate(
            icon = Octicons.Repo,
            title = "No repositories",
            description = "Add a repository from this device's storage.",
            primaryAction = { PrimerButton("Add repository", {}, variant = PrimerButtonVariant.Primary) },
        )
    }
}
