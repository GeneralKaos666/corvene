package com.wasimaster.corvene.design

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * One Primer semantic hue: [fg] for text and icons, [emphasis] for solid
 * fills (with `textOnEmphasis` on top), [muted] for borders, [subtle] for
 * tinted backgrounds (Primer's `fgColor-x`, `bgColor-x-emphasis`,
 * `borderColor-x-muted`, `bgColor-x-muted`).
 */
@Immutable
data class SemanticColor(
    val fg: Color,
    val emphasis: Color,
    val muted: Color,
    val subtle: Color,
)

/**
 * The functional colour tokens every component draws with, one instance per
 * (style, light/dark, contrast). The two GitHub styles' values are generated
 * (tools/tokens/gen.py → Palettes.kt); Material's are derived from the M3
 * scheme ([materialPrimerColors]). Read through [CorveneTheme.colors].
 */
@Immutable
data class PrimerColors(
    val isDark: Boolean,
    // text
    val textPrimary: Color,
    val textSecondary: Color,
    val textTertiary: Color,
    val textPlaceholder: Color,
    val textLink: Color,
    val textOnEmphasis: Color,
    val textDisabled: Color,
    // icons (Primer Mobile's rules: link blue for chrome and the view's primary action)
    val iconPrimary: Color,
    val iconSecondary: Color,
    val iconLink: Color,
    // backgrounds
    val bgCanvas: Color,
    val bgDefault: Color,
    val bgInset: Color,
    val bgSubtle: Color,
    val bgOverlay: Color,
    val bgSelected: Color,
    val bgSelectedActive: Color,
    val bgHover: Color,
    // borders
    val borderDefault: Color,
    val borderMuted: Color,
    val borderEmphasis: Color,
    // semantic hues
    val accent: SemanticColor,
    val success: SemanticColor,
    val attention: SemanticColor,
    val danger: SemanticColor,
    val done: SemanticColor,
    val sponsors: SemanticColor,
    // file status (GHD's --color-new / deleted / modified / renamed / conflicted)
    val fileNew: Color,
    val fileDeleted: Color,
    val fileModified: Color,
    val fileRenamed: Color,
    val fileConflicted: Color,
    // diff
    val diffAddBg: Color,
    val diffAddGutterBg: Color,
    val diffAddWordBg: Color,
    val diffAddBorder: Color,
    val diffDelBg: Color,
    val diffDelGutterBg: Color,
    val diffDelWordBg: Color,
    val diffDelBorder: Color,
    val diffHunkBg: Color,
    val diffHunkBorder: Color,
    val diffHunkText: Color,
    val diffGutterBg: Color,
    val diffLineNumber: Color,
    val diffText: Color,
    val diffAltText: Color,
    val diffSelectedBg: Color,
    val diffSelectedText: Color,
    val diffHoverBg: Color,
    val diffEmptyRowBg: Color,
    // syntax, one per corvene_highlight::TokenClass
    val syntaxVariable: Color,
    val syntaxAltVariable: Color,
    val syntaxKeyword: Color,
    val syntaxAtom: Color,
    val syntaxString: Color,
    val syntaxQualifier: Color,
    val syntaxType: Color,
    val syntaxComment: Color,
    val syntaxTag: Color,
    val syntaxAttribute: Color,
    val syntaxLink: Color,
    val syntaxHeader: Color,
    val syntaxQuote: Color,
    // chrome
    val toolbarBg: Color,
    val toolbarText: Color,
    val toolbarTextSecondary: Color,
    val toolbarButtonBorder: Color,
    val toolbarButtonHoverBg: Color,
    val tabBarActive: Color,
    val tabBarCountBg: Color,
    val badgeBg: Color,
    val badgeText: Color,
    val coAuthorTagBg: Color,
    val coAuthorTagBorder: Color,
    // transient surfaces
    val toastBg: Color,
    val toastText: Color,
    val scrim: Color,
)

/** The generated palette of a GitHub style. Material has none: it is derived from the scheme. */
internal fun githubPalette(style: DesignStyle, dark: Boolean, highContrast: Boolean): PrimerColors = when (style) {
    DesignStyle.GitHubDesktop -> when {
        // GHD's high contrast theme is dark-only; light keeps Primer's light-hc
        highContrast && dark -> GitHubDesktopHighContrast
        highContrast -> GitHubMobileLightHighContrast
        dark -> GitHubDesktopDark
        else -> GitHubDesktopLight
    }
    DesignStyle.GitHubMobile, DesignStyle.Material -> when {
        highContrast && dark -> GitHubMobileDarkHighContrast
        highContrast -> GitHubMobileLightHighContrast
        dark -> GitHubMobileDark
        else -> GitHubMobileLight
    }
}
