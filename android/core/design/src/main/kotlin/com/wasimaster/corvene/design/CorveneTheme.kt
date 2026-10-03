package com.wasimaster.corvene.design

import android.app.UiModeManager
import android.content.Context
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext

/**
 * The style in effect. Internal on purpose: screens never branch on the style
 * themselves; they use the components here, which do (Konsist checks that
 * nothing outside :core:design names it).
 */
internal val LocalDesignStyle = staticCompositionLocalOf { DesignStyle.Default }
internal val LocalPrimerColors = staticCompositionLocalOf { GitHubMobileLight }
internal val LocalCorveneSpacing = staticCompositionLocalOf { spacingFor(DesignStyle.Default) }
internal val LocalCorveneMetrics = staticCompositionLocalOf { metricsFor(DesignStyle.Default) }
internal val LocalCorveneTextStyles = staticCompositionLocalOf { textStylesFor(DesignStyle.Default) }

/**
 * Corvene's theme: [style] × light/dark ([colorMode]) × contrast.
 *
 * [highContrast] null follows the system (Settings › Accessibility ›
 * High contrast text, or the Android 14 contrast slider at ≥ 0.5).
 * [dynamicColor] applies to the Material style on Android 12+.
 * Provides MaterialTheme (colour scheme mapped from the Primer tokens for the
 * two GitHub styles), and Corvene's own tokens through [CorveneTheme].
 */
@Composable
fun CorveneTheme(
    style: DesignStyle,
    colorMode: ColorMode = ColorMode.System,
    highContrast: Boolean? = null,
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val configuration = LocalConfiguration.current
    val systemDark = isSystemInDarkTheme()
    val dark = when (colorMode) {
        ColorMode.System -> systemDark
        ColorMode.Light -> false
        ColorMode.Dark -> true
    }
    val contrast = highContrast ?: remember(context, configuration) { systemHighContrast(context) }
    // Remembered: ColorScheme compares by instance, and a fresh equal one
    // would still recompose everything under MaterialTheme.
    val (scheme, colors) = remember(style, dark, contrast, dynamicColor, context, configuration) {
        if (style == DesignStyle.Material) {
            val scheme = materialScheme(context, dark, dynamicColor)
            scheme to materialPrimerColors(scheme, dark)
        } else {
            val colors = githubPalette(style, dark, contrast)
            colors.toColorScheme() to colors
        }
    }
    MaterialTheme(colorScheme = scheme, typography = typographyFor(style), shapes = shapesFor(style)) {
        CompositionLocalProvider(
            LocalDesignStyle provides style,
            LocalPrimerColors provides colors,
            LocalCorveneSpacing provides spacingFor(style),
            LocalCorveneMetrics provides metricsFor(style),
            LocalCorveneTextStyles provides textStylesFor(style),
            content = content,
        )
    }
}

/** Corvene's tokens, the way `MaterialTheme.colorScheme` reads M3's. */
object CorveneTheme {
    val colors: PrimerColors
        @Composable @ReadOnlyComposable
        get() = LocalPrimerColors.current

    val spacing: CorveneSpacing
        @Composable @ReadOnlyComposable
        get() = LocalCorveneSpacing.current

    val metrics: CorveneMetrics
        @Composable @ReadOnlyComposable
        get() = LocalCorveneMetrics.current

    val textStyles: CorveneTextStyles
        @Composable @ReadOnlyComposable
        get() = LocalCorveneTextStyles.current
}

private fun materialScheme(context: Context, dark: Boolean, dynamic: Boolean): ColorScheme = when {
    dynamic && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
        if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    dark -> darkColorScheme()
    else -> lightColorScheme()
}

private fun systemHighContrast(context: Context): Boolean {
    val text = Settings.Secure.getInt(context.contentResolver, HIGH_TEXT_CONTRAST, 0) == 1
    val slider = Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE &&
        (context.getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager)?.contrast?.let { it >= HIGH_CONTRAST_LEVEL } == true
    return text || slider
}

/** `Settings.Secure.ACCESSIBILITY_HIGH_TEXT_CONTRAST_ENABLED`, hidden API but stable since Lollipop. */
private const val HIGH_TEXT_CONTRAST = "high_text_contrast_enabled"
private const val HIGH_CONTRAST_LEVEL = 0.5f
