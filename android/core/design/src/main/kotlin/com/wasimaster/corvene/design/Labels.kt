package com.wasimaster.corvene.design

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Primer's CounterLabel: a small count pill ("3", "12↑"). */
@Composable
fun CounterLabel(text: String, modifier: Modifier = Modifier, emphasis: Boolean = false) {
    val colors = CorveneTheme.colors
    Text(
        text,
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(if (emphasis) colors.accent.emphasis else colors.badgeBg)
            .defaultMinSize(minWidth = 20.dp)
            .padding(horizontal = 6.dp, vertical = 1.dp),
        style = MaterialTheme.typography.labelMedium,
        color = if (emphasis) colors.textOnEmphasis else colors.badgeText,
        textAlign = TextAlign.Center,
        maxLines = 1,
    )
}

/**
 * An avatar placeholder: the first letter of [name] in a circle. M-A1 draws
 * the image the engine fetched instead (a file path through Coil).
 */
@Composable
fun Avatar(name: String, modifier: Modifier = Modifier, size: Dp = 32.dp) {
    val colors = CorveneTheme.colors
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(colors.bgSubtle)
            .border(1.dp, colors.borderMuted, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            name.firstOrNull()?.uppercase() ?: "?",
            style = if (size >= 32.dp) MaterialTheme.typography.titleMedium else MaterialTheme.typography.labelSmall,
            color = colors.textSecondary,
        )
    }
}

@Preview(widthDp = 240)
@Composable
private fun LabelsPreview() {
    DesignStyleSamples {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar("wasi-master")
            Avatar("octocat", size = 20.dp)
            CounterLabel("3")
            CounterLabel("12", emphasis = true)
        }
    }
}
