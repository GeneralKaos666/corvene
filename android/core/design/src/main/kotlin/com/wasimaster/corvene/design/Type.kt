@file:OptIn(ExperimentalTextApi::class)

package com.wasimaster.corvene.design

import androidx.compose.material3.Typography
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.runtime.Immutable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp

/** Inter (variable, OFL), GitHub Mobile's interface face on Android. */
private val Inter = FontFamily(
    listOf(400, 500, 600, 700).map { weight ->
        Font(
            R.font.cvd_inter_variable,
            weight = FontWeight(weight),
            variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
        )
    },
)

/**
 * JetBrains Mono (OFL), for diffs and code. The NL cut: no programming
 * ligatures, so `!=` stays two characters like the file has.
 */
val JetBrainsMono = FontFamily(
    Font(R.font.cvd_jetbrains_mono_regular, FontWeight.Normal, FontStyle.Normal),
    Font(R.font.cvd_jetbrains_mono_italic, FontWeight.Normal, FontStyle.Italic),
)

/** Text styles M3's [Typography] has no slot for. */
@Immutable
data class CorveneTextStyles(
    /** Diff lines and code. */
    val code: TextStyle,
    /** Paths, hashes and branch names inline in UI text. */
    val codeSmall: TextStyle,
)

private fun style(family: FontFamily, size: Int, line: Int, weight: Int = 400, tracking: TextUnit = 0.sp) = TextStyle(
    fontFamily = family,
    fontSize = size.sp,
    lineHeight = line.sp,
    fontWeight = FontWeight(weight),
    letterSpacing = tracking,
)

/** GitHub Mobile: the M3 scale set in Inter, titles and labels semibold. */
private val MobileTypography = Typography(
    displayLarge = style(Inter, 57, 64, 500),
    displayMedium = style(Inter, 45, 52, 500),
    displaySmall = style(Inter, 36, 44, 600),
    headlineLarge = style(Inter, 32, 40, 600),
    headlineMedium = style(Inter, 28, 36, 600),
    headlineSmall = style(Inter, 24, 32, 600),
    titleLarge = style(Inter, 22, 28, 600),
    titleMedium = style(Inter, 16, 24, 600),
    titleSmall = style(Inter, 14, 20, 600),
    bodyLarge = style(Inter, 16, 24),
    bodyMedium = style(Inter, 14, 20),
    bodySmall = style(Inter, 12, 16),
    labelLarge = style(Inter, 14, 20, 600),
    labelMedium = style(Inter, 12, 16, 600),
    labelSmall = style(Inter, 11, 16, 500),
)

/**
 * GitHub Desktop: the system face at GHD's sizes (`_variables.scss`: xs 9,
 * sm 11, base 12, md 14, lg 28, xl 32, xxl 42), as sp.
 */
private val DesktopTypography = FontFamily.Default.let { sans ->
    Typography(
        displayLarge = style(sans, 42, 52, 300),
        displayMedium = style(sans, 42, 52, 300),
        displaySmall = style(sans, 32, 40, 300),
        headlineLarge = style(sans, 32, 40, 400),
        headlineMedium = style(sans, 28, 36, 400),
        headlineSmall = style(sans, 20, 28, 600),
        titleLarge = style(sans, 16, 24, 600),
        titleMedium = style(sans, 14, 20, 600),
        titleSmall = style(sans, 12, 18, 600),
        bodyLarge = style(sans, 14, 20),
        bodyMedium = style(sans, 12, 18),
        bodySmall = style(sans, 11, 16),
        labelLarge = style(sans, 12, 18, 600),
        labelMedium = style(sans, 11, 16, 600),
        labelSmall = style(sans, 9, 12, 600),
    )
}

internal fun typographyFor(style: DesignStyle): Typography = when (style) {
    DesignStyle.GitHubMobile -> MobileTypography
    DesignStyle.GitHubDesktop -> DesktopTypography
    DesignStyle.Material -> Typography()
}

internal fun textStylesFor(style: DesignStyle): CorveneTextStyles = when (style) {
    DesignStyle.GitHubDesktop -> CorveneTextStyles(code = style(JetBrainsMono, 12, 18), codeSmall = style(JetBrainsMono, 11, 16))
    DesignStyle.GitHubMobile, DesignStyle.Material ->
        CorveneTextStyles(code = style(JetBrainsMono, 13, 20), codeSmall = style(JetBrainsMono, 12, 16))
}
