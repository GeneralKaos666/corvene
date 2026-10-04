package com.wasimaster.corvene.design

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.lerp

/**
 * The Material style's tokens, from its M3 scheme (dynamic on Android 12+):
 * text and surfaces are the scheme's roles, Primer's semantic hues are kept but
 * pulled 15 % toward the primary colour, and diff fills run at 60 % so the
 * wallpaper's tint shows through.
 */
internal fun materialPrimerColors(scheme: ColorScheme, dark: Boolean): PrimerColors {
    val base = if (dark) GitHubMobileDark else GitHubMobileLight
    fun tint(c: Color) = lerp(c, scheme.primary, TINT)
    fun tint(s: SemanticColor) = SemanticColor(tint(s.fg), tint(s.emphasis), tint(s.muted), tint(s.subtle))
    fun diff(c: Color) = c.copy(alpha = c.alpha * DIFF_ALPHA)
    return base.copy(
        textPrimary = scheme.onSurface,
        textSecondary = scheme.onSurfaceVariant,
        textTertiary = scheme.onSurfaceVariant.copy(alpha = 0.7f),
        textPlaceholder = scheme.onSurfaceVariant,
        textLink = scheme.primary,
        textOnEmphasis = scheme.onPrimary,
        textDisabled = scheme.onSurface.copy(alpha = 0.38f),
        iconPrimary = scheme.onSurfaceVariant,
        iconSecondary = scheme.outline,
        iconLink = scheme.primary,
        bgCanvas = scheme.surface,
        bgDefault = scheme.surface,
        bgInset = scheme.surfaceContainerLowest,
        bgSubtle = scheme.surfaceContainerLow,
        bgOverlay = scheme.surfaceContainerHigh,
        bgSelected = scheme.secondaryContainer,
        bgSelectedActive = scheme.primary,
        bgHover = scheme.onSurface.copy(alpha = 0.08f),
        borderDefault = scheme.outlineVariant,
        borderMuted = scheme.outlineVariant.copy(alpha = 0.6f),
        borderEmphasis = scheme.outline,
        accent = SemanticColor(scheme.primary, scheme.primary, scheme.primary.copy(alpha = 0.4f), scheme.primaryContainer),
        success = tint(base.success),
        attention = tint(base.attention),
        danger = SemanticColor(scheme.error, scheme.error, scheme.error.copy(alpha = 0.4f), scheme.errorContainer),
        done = tint(base.done),
        sponsors = tint(base.sponsors),
        fileNew = tint(base.fileNew),
        fileDeleted = tint(base.fileDeleted),
        fileModified = tint(base.fileModified),
        fileRenamed = scheme.primary,
        fileConflicted = tint(base.fileConflicted),
        diffAddBg = diff(base.diffAddBg),
        diffAddGutterBg = diff(base.diffAddGutterBg),
        diffAddWordBg = diff(base.diffAddWordBg),
        diffDelBg = diff(base.diffDelBg),
        diffDelGutterBg = diff(base.diffDelGutterBg),
        diffDelWordBg = diff(base.diffDelWordBg),
        diffHunkBg = scheme.primaryContainer.copy(alpha = DIFF_ALPHA),
        diffHunkText = scheme.onSurfaceVariant,
        diffGutterBg = scheme.surface,
        diffLineNumber = scheme.onSurfaceVariant,
        diffText = scheme.onSurface,
        diffAltText = scheme.onSurfaceVariant,
        diffSelectedBg = scheme.primary,
        diffSelectedText = scheme.onPrimary,
        diffEmptyRowBg = scheme.surfaceContainerLow,
        toolbarBg = scheme.surfaceContainer,
        toolbarText = scheme.onSurface,
        toolbarTextSecondary = scheme.onSurfaceVariant,
        toolbarButtonBorder = scheme.outlineVariant,
        toolbarButtonHoverBg = scheme.onSurface.copy(alpha = 0.08f),
        tabBarActive = scheme.primary,
        tabBarCountBg = scheme.secondaryContainer,
        badgeBg = scheme.secondaryContainer,
        badgeText = scheme.onSecondaryContainer,
        coAuthorTagBg = scheme.primaryContainer,
        coAuthorTagBorder = scheme.primary.copy(alpha = 0.4f),
        toastBg = scheme.inverseSurface,
        toastText = scheme.inverseOnSurface,
        scrim = scheme.scrim.copy(alpha = if (dark) 0.5f else 0.4f),
    )
}

/**
 * The M3 scheme of a GitHub style, so stock Material components (menus,
 * dialogs, sheets, text fields) wear Primer colours. Containers are composited
 * over the canvas: Primer's translucent fills would show what is behind a sheet.
 */
internal fun PrimerColors.toColorScheme(): ColorScheme {
    fun solid(c: Color) = c.compositeOver(bgCanvas)
    val base = if (isDark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = accent.emphasis,
        onPrimary = textOnEmphasis,
        primaryContainer = solid(accent.subtle),
        onPrimaryContainer = accent.fg,
        inversePrimary = solid(accent.muted),
        secondary = textSecondary,
        onSecondary = bgCanvas,
        secondaryContainer = solid(bgSubtle),
        onSecondaryContainer = textPrimary,
        tertiary = done.fg,
        onTertiary = textOnEmphasis,
        tertiaryContainer = solid(done.subtle),
        onTertiaryContainer = done.fg,
        background = bgCanvas,
        onBackground = textPrimary,
        surface = bgDefault,
        onSurface = textPrimary,
        surfaceVariant = solid(bgSubtle),
        onSurfaceVariant = textSecondary,
        surfaceTint = bgDefault,
        inverseSurface = toastBg,
        inverseOnSurface = toastText,
        error = danger.fg,
        onError = textOnEmphasis,
        errorContainer = solid(danger.subtle),
        onErrorContainer = danger.fg,
        outline = solid(borderDefault),
        outlineVariant = solid(borderMuted),
        scrim = scrim,
        surfaceBright = bgDefault,
        surfaceContainer = solid(bgSubtle),
        surfaceContainerHigh = solid(bgOverlay),
        surfaceContainerHighest = solid(bgSelected),
        surfaceContainerLow = solid(bgSubtle),
        surfaceContainerLowest = solid(bgInset),
        surfaceDim = solid(bgInset),
    )
}

private const val TINT = 0.15f
private const val DIFF_ALPHA = 0.6f
