package com.wasimaster.corvene.settings

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.DesignStyleVm
import com.wasimaster.corvene.ffi.gen.SettingsVm
import com.wasimaster.corvene.ffi.gen.ThemeVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private val Settings = SettingsVm(
    designStyle = DesignStyleVm.GIT_HUB_MOBILE,
    designStyleSetting = DesignStyleVm.GIT_HUB_MOBILE,
    designStylePinned = false,
    theme = ThemeVm.SYSTEM,
    welcomeCompleted = true,
    confirmDiscardChanges = true,
    confirmForcePush = true,
    confirmRepositoryRemoval = true,
    notificationsEnabled = true,
    repositoryIndicatorsEnabled = true,
    hideWhitespaceInChangesDiff = false,
    hideWhitespaceInHistoryDiff = false,
    showDiffCheckMarks = true,
    underlineLinks = true,
)

/** Settings › Appearance: the cards and radios send the engine's values; screenshots in every style. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class AppearanceScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val styles = mutableListOf<DesignStyleVm>()
    private val themes = mutableListOf<ThemeVm>()

    private fun show(style: DesignStyle, mode: ColorMode, settings: SettingsVm = Settings.copy(designStyleSetting = style.toVm())) {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) {
                AppearanceScreen(settings, dark = mode == ColorMode.Dark, onStyle = { styles += it }, onTheme = { themes += it })
            }
        }
    }

    private fun shoot(style: DesignStyle, mode: ColorMode) {
        show(style, mode)
        compose.onRoot().captureRoboImage("src/test/screenshots/appearance_${style.key}_${mode.key}.png")
    }

    @Test
    fun `cards and theme rows send their values`() {
        show(DesignStyle.GitHubMobile, ColorMode.Light)
        compose.onNodeWithTag("${TAG_STYLE}MATERIAL").performClick()
        compose.onNodeWithTag("${TAG_THEME}DARK").performClick()
        assertEquals(listOf(DesignStyleVm.MATERIAL), styles)
        assertEquals(listOf(ThemeVm.DARK), themes)
    }

    @Test
    fun `a pinned style says so and ignores taps`() {
        show(DesignStyle.GitHubDesktop, ColorMode.Light, Settings.copy(designStylePinned = true))
        compose.onNodeWithText("pinned by flag", substring = true).assertExists()
        compose.onNodeWithTag("${TAG_STYLE}MATERIAL").performClick()
        assertEquals(emptyList<DesignStyleVm>(), styles)
    }

    @Test fun mobileLight() = shoot(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun mobileDark() = shoot(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun desktopLight() = shoot(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun desktopDark() = shoot(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun materialLight() = shoot(DesignStyle.Material, ColorMode.Light)

    @Test fun materialDark() = shoot(DesignStyle.Material, ColorMode.Dark)
}
