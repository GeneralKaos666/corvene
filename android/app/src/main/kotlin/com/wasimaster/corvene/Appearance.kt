package com.wasimaster.corvene

import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.SettingsVm
import com.wasimaster.corvene.ffi.gen.ThemeVm
import com.wasimaster.corvene.settings.toDesignStyle

/**
 * How the app looks, as the engine's settings say (Settings › Appearance;
 * `design_style` may be pinned by flag `113-design-style`). [highContrast]
 * null follows the system's high-contrast text setting.
 */
data class Appearance(
    val style: DesignStyle,
    val colorMode: ColorMode,
    val highContrast: Boolean?,
    val styleSetting: DesignStyle,
    val stylePinned: Boolean,
    val theme: ThemeVm,
)

fun SettingsVm.toAppearance(): Appearance = Appearance(
    style = designStyle.toDesignStyle(),
    colorMode = when (theme) {
        ThemeVm.LIGHT -> ColorMode.Light
        ThemeVm.DARK, ThemeVm.HIGH_CONTRAST -> ColorMode.Dark
        ThemeVm.SYSTEM -> ColorMode.System
    },
    // GHD's High Contrast theme is a dark one; the other themes follow the system's contrast setting
    highContrast = if (theme == ThemeVm.HIGH_CONTRAST) true else null,
    styleSetting = designStyleSetting.toDesignStyle(),
    stylePinned = designStylePinned,
    theme = theme,
)
