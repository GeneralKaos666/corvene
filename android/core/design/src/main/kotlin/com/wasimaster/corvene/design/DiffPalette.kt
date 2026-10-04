package com.wasimaster.corvene.design

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * What a diff is drawn with: the row fills, gutters and the syntax colours
 * of the style in effect ([PrimerColors]' diff and syntax tokens), on their
 * own so a diff row reads one small object instead of the whole palette.
 * Provided by [CorveneTheme] as [LocalDiffPalette]; caches of highlighted
 * lines key on the instance (it changes only with the theme).
 */
@Immutable
data class DiffPalette(
    val isDark: Boolean,
    val text: Color,
    val altText: Color,
    val canvas: Color,
    val addBg: Color,
    val addGutterBg: Color,
    val addWordBg: Color,
    val addSign: Color,
    val delBg: Color,
    val delGutterBg: Color,
    val delWordBg: Color,
    val delSign: Color,
    val hunkBg: Color,
    val hunkBorder: Color,
    val hunkText: Color,
    val gutterBg: Color,
    val lineNumber: Color,
    val selectedBg: Color,
    val selectedText: Color,
    val emptyRowBg: Color,
    val border: Color,
    val syntax: SyntaxColors,
)

/** One colour per `corvene_highlight::TokenClass`. */
@Immutable
data class SyntaxColors(
    val variable: Color,
    val altVariable: Color,
    val keyword: Color,
    val atom: Color,
    val string: Color,
    val qualifier: Color,
    val type: Color,
    val comment: Color,
    val tag: Color,
    val attribute: Color,
    val link: Color,
    val header: Color,
    val quote: Color,
)

internal fun PrimerColors.toDiffPalette(): DiffPalette = DiffPalette(
    isDark = isDark,
    text = diffText,
    altText = diffAltText,
    canvas = bgCanvas,
    addBg = diffAddBg,
    addGutterBg = diffAddGutterBg,
    addWordBg = diffAddWordBg,
    addSign = success.fg,
    delBg = diffDelBg,
    delGutterBg = diffDelGutterBg,
    delWordBg = diffDelWordBg,
    delSign = danger.fg,
    hunkBg = diffHunkBg,
    hunkBorder = diffHunkBorder,
    hunkText = diffHunkText,
    gutterBg = diffGutterBg,
    lineNumber = diffLineNumber,
    selectedBg = diffSelectedBg,
    selectedText = diffSelectedText,
    emptyRowBg = diffEmptyRowBg,
    border = borderMuted,
    syntax = SyntaxColors(
        variable = syntaxVariable,
        altVariable = syntaxAltVariable,
        keyword = syntaxKeyword,
        atom = syntaxAtom,
        string = syntaxString,
        qualifier = syntaxQualifier,
        type = syntaxType,
        comment = syntaxComment,
        tag = syntaxTag,
        attribute = syntaxAttribute,
        link = syntaxLink,
        header = syntaxHeader,
        quote = syntaxQuote,
    ),
)

/** The diff colours of the theme in effect. */
val LocalDiffPalette = staticCompositionLocalOf { GitHubMobileLight.toDiffPalette() }

/**
 * The tint an [Octicon] takes when its caller names none: [OcticonTint.Primary]
 * in content, [OcticonTint.Link] inside app chrome (Primer Mobile: bar icons
 * are link blue). Bars provide it for their actions.
 */
val LocalOcticonTint = staticCompositionLocalOf { OcticonTint.Primary }

/**
 * The diff colours of [style] outside a composition (tests, benchmarks);
 * Material uses the stock, not the dynamic, scheme.
 */
fun diffPaletteOf(style: DesignStyle, dark: Boolean, highContrast: Boolean = false): DiffPalette = when (style) {
    DesignStyle.Material -> materialPrimerColors(if (dark) darkColorScheme() else lightColorScheme(), dark).toDiffPalette()
    DesignStyle.GitHubMobile, DesignStyle.GitHubDesktop -> githubPalette(style, dark, highContrast).toDiffPalette()
}
