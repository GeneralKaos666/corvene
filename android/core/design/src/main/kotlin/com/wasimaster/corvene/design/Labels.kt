package com.wasimaster.corvene.design

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import java.io.File

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

/** Primer's Label variants (metadata chips). */
enum class LabelVariant { Default, Accent, Success, Attention, Danger, Done }

/** Primer's Label: a small outlined chip for metadata ("Partial", "Conflicted", "LFS"). */
@Composable
fun Label(text: String, modifier: Modifier = Modifier, variant: LabelVariant = LabelVariant.Default) {
    val colors = CorveneTheme.colors
    val (fg, border) = when (variant) {
        LabelVariant.Default -> colors.textSecondary to colors.borderDefault
        LabelVariant.Accent -> colors.accent.fg to colors.accent.muted
        LabelVariant.Success -> colors.success.fg to colors.success.muted
        LabelVariant.Attention -> colors.attention.fg to colors.attention.muted
        LabelVariant.Danger -> colors.danger.fg to colors.danger.muted
        LabelVariant.Done -> colors.done.fg to colors.done.muted
    }
    Text(
        text,
        modifier = modifier
            .border(1.dp, border, RoundedCornerShape(50))
            .padding(horizontal = 7.dp, vertical = 1.dp),
        style = MaterialTheme.typography.labelMedium,
        color = fg,
        maxLines = 1,
    )
}

/** The state of a [StateLabel]: open = success, closed = danger, merged = done, draft = neutral. */
enum class StateLabelState { Open, Closed, Merged, Draft }

/** Primer's StateLabel: a filled pill with an icon for an issue/PR state or a conflict. */
@Composable
fun StateLabel(text: String, state: StateLabelState, modifier: Modifier = Modifier, icon: OcticonIcon? = null) {
    val colors = CorveneTheme.colors
    val bg = when (state) {
        StateLabelState.Open -> colors.success.emphasis
        StateLabelState.Closed -> colors.danger.emphasis
        StateLabelState.Merged -> colors.done.emphasis
        StateLabelState.Draft -> colors.textSecondary
    }
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(bg)
            .padding(horizontal = 8.dp, vertical = 3.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) OcticonColored(icon, null, colors.textOnEmphasis, size = 14.dp)
        Text(text, style = MaterialTheme.typography.labelMedium, color = colors.textOnEmphasis, maxLines = 1)
    }
}

/**
 * Primer's BranchName: a monospace chip in the accent colours, with the
 * `git-branch` icon when shown on its own ([icon]).
 */
@Composable
fun BranchName(name: String, modifier: Modifier = Modifier, icon: Boolean = false) {
    val colors = CorveneTheme.colors
    Row(
        modifier
            .clip(RoundedCornerShape(CorveneTheme.metrics.cornerMedium))
            .background(colors.accent.subtle)
            .padding(horizontal = 6.dp, vertical = 2.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon) OcticonColored(Octicons.GitBranch, null, colors.accent.fg, size = 14.dp)
        Text(
            name,
            style = CorveneTheme.textStyles.codeSmall,
            color = colors.accent.fg,
            maxLines = 1,
            overflow = TextOverflow.MiddleEllipsis,
        )
    }
}

/**
 * An avatar: the image the engine downloaded ([path], a file; Coil decodes
 * it, nothing here touches the network), over the first letter of [name] in
 * a circle, which is what shows while it loads, when it fails or when there
 * is none. [square] for organisations and bots (Primer).
 */
@Composable
fun Avatar(name: String, modifier: Modifier = Modifier, size: Dp = 32.dp, path: String? = null, square: Boolean = false) {
    val colors = CorveneTheme.colors
    val shape = if (square) RoundedCornerShape(CorveneTheme.metrics.cornerMedium) else CircleShape
    Box(
        modifier
            .size(size)
            .clip(shape)
            .background(colors.bgSubtle)
            .border(1.dp, colors.borderMuted, shape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            name.firstOrNull()?.uppercase() ?: "?",
            style = if (size >= 32.dp) MaterialTheme.typography.titleMedium else MaterialTheme.typography.labelSmall,
            color = colors.textSecondary,
        )
        if (path != null) {
            AsyncImage(
                model = File(path),
                contentDescription = null,
                imageLoader = avatarLoader(LocalContext.current),
                contentScale = ContentScale.Crop,
                modifier = Modifier.matchParentSize(),
            )
        }
    }
}

/** One image loader for every avatar: files only, memory cache, no network fetcher on the classpath. */
private fun avatarLoader(context: Context): ImageLoader =
    AvatarLoader.instance ?: synchronized(AvatarLoader) {
        AvatarLoader.instance ?: ImageLoader.Builder(context.applicationContext).build().also { AvatarLoader.instance = it }
    }

private object AvatarLoader {
    @Volatile
    var instance: ImageLoader? = null
}

@Preview(widthDp = 300, heightDp = 700)
@Composable
private fun LabelsPreview() {
    DesignStyleSamples {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar("wasi-master")
            Avatar("octocat", size = 20.dp)
            Avatar("github", square = true)
            CounterLabel("3")
            CounterLabel("12", emphasis = true)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Label("Partial", variant = LabelVariant.Accent)
            Label("LFS")
            StateLabel("Conflicts", StateLabelState.Closed, icon = Octicons.Alert)
        }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            BranchName("main", icon = true)
            StateLabel("Merged", StateLabelState.Merged, icon = Octicons.GitMerge)
        }
    }
}
