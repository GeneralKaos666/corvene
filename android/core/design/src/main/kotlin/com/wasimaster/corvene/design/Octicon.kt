package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** One Octicon at its two drawn sizes (see [Octicons], generated). */
@Immutable
class OcticonIcon(
    val name: String,
    /** The 16 px drawable. */
    val small: Int,
    /** The 24 px drawable. */
    val large: Int,
)

/**
 * The role an icon plays, which decides its colour (Primer Mobile's rules):
 * [Link] for app chrome and the view's primary action, [Primary] for
 * actionable or primary supporting icons, [Secondary] for decoration, and the
 * semantic hues when the icon carries state.
 */
enum class OcticonTint { Primary, Secondary, Link, Success, Danger, Attention, Done, OnEmphasis }

@Composable
@ReadOnlyComposable
fun OcticonTint.color(): Color {
    val c = CorveneTheme.colors
    return when (this) {
        OcticonTint.Primary -> c.iconPrimary
        OcticonTint.Secondary -> c.iconSecondary
        OcticonTint.Link -> c.iconLink
        OcticonTint.Success -> c.success.fg
        OcticonTint.Danger -> c.danger.fg
        OcticonTint.Attention -> c.attention.fg
        OcticonTint.Done -> c.done.fg
        OcticonTint.OnEmphasis -> c.textOnEmphasis
    }
}

/**
 * An Octicon: the 24 px drawing at 24 dp and up, the 16 px one below. The
 * tint defaults to [LocalOcticonTint] (link blue inside bars).
 */
@Composable
fun Octicon(
    icon: OcticonIcon,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    tint: OcticonTint = LocalOcticonTint.current,
    size: Dp = 16.dp,
) {
    OcticonColored(icon, contentDescription, tint.color(), modifier, size)
}

/** An Octicon in an explicit colour (a row's text colour, a file status). */
@Composable
fun OcticonColored(
    icon: OcticonIcon,
    contentDescription: String?,
    color: Color,
    modifier: Modifier = Modifier,
    size: Dp = 16.dp,
) {
    Icon(
        painter = painterResource(if (size >= 24.dp) icon.large else icon.small),
        contentDescription = contentDescription,
        modifier = modifier.size(size),
        tint = color,
    )
}
