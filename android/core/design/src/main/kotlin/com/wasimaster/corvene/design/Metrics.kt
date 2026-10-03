package com.wasimaster.corvene.design

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.runtime.Immutable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The spacing scale of a style: Primer's 8-pt grid, GHD's 10 px one. */
@Immutable
data class CorveneSpacing(
    val xxs: Dp,
    val xs: Dp,
    val s: Dp,
    val m: Dp,
    val l: Dp,
    val xl: Dp,
)

/** Sizes components agree on, per style. */
@Immutable
data class CorveneMetrics(
    /** A one-line list row. */
    val rowHeight: Dp,
    /** A two-line list row. */
    val rowHeightLarge: Dp,
    /** A button or field. */
    val controlHeight: Dp,
    /** The smallest touch target, whatever is drawn. */
    val minTouchTarget: Dp,
    /** Horizontal padding of rows and screens. */
    val gutter: Dp,
    /** The leading icon tile of a top-level row (GitHub Mobile's coloured squares). */
    val iconTile: Dp,
    /** Dividers start after the leading visual (Mobile, Material) or run full width (Desktop). */
    val insetDividers: Boolean,
    val cornerSmall: Dp,
    val cornerMedium: Dp,
    val cornerLarge: Dp,
)

internal fun spacingFor(style: DesignStyle): CorveneSpacing = when (style) {
    DesignStyle.GitHubDesktop -> CorveneSpacing(xxs = 3.dp, xs = 5.dp, s = 10.dp, m = 15.dp, l = 20.dp, xl = 30.dp)
    DesignStyle.GitHubMobile, DesignStyle.Material -> CorveneSpacing(xxs = 2.dp, xs = 4.dp, s = 8.dp, m = 16.dp, l = 24.dp, xl = 32.dp)
}

internal fun metricsFor(style: DesignStyle): CorveneMetrics = when (style) {
    DesignStyle.GitHubMobile -> CorveneMetrics(
        rowHeight = 56.dp, rowHeightLarge = 72.dp, controlHeight = 40.dp, minTouchTarget = 48.dp, gutter = 16.dp,
        iconTile = 32.dp, insetDividers = true, cornerSmall = 6.dp, cornerMedium = 6.dp, cornerLarge = 12.dp,
    )
    DesignStyle.GitHubDesktop -> CorveneMetrics(
        rowHeight = 40.dp, rowHeightLarge = 56.dp, controlHeight = 32.dp, minTouchTarget = 48.dp, gutter = 10.dp,
        iconTile = 24.dp, insetDividers = false, cornerSmall = 3.dp, cornerMedium = 6.dp, cornerLarge = 6.dp,
    )
    DesignStyle.Material -> CorveneMetrics(
        rowHeight = 56.dp, rowHeightLarge = 72.dp, controlHeight = 40.dp, minTouchTarget = 48.dp, gutter = 16.dp,
        iconTile = 40.dp, insetDividers = true, cornerSmall = 8.dp, cornerMedium = 12.dp, cornerLarge = 16.dp,
    )
}

internal fun shapesFor(style: DesignStyle): Shapes = when (style) {
    DesignStyle.GitHubMobile -> Shapes(
        extraSmall = RoundedCornerShape(4.dp),
        small = RoundedCornerShape(6.dp),
        medium = RoundedCornerShape(12.dp),
        large = RoundedCornerShape(12.dp),
        extraLarge = RoundedCornerShape(16.dp),
    )
    DesignStyle.GitHubDesktop -> Shapes(
        extraSmall = RoundedCornerShape(3.dp),
        small = RoundedCornerShape(3.dp),
        medium = RoundedCornerShape(6.dp),
        large = RoundedCornerShape(6.dp),
        extraLarge = RoundedCornerShape(6.dp),
    )
    DesignStyle.Material -> Shapes()
}
