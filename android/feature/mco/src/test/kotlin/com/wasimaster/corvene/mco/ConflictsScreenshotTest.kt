package com.wasimaster.corvene.mco

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The conflicts screen in every style, light and dark, on a phone. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class ConflictsScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(style: DesignStyle, mode: ColorMode) {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) { ConflictsScreen(SampleConflicts, NoConflictsActions) }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/conflicts_${style.key}_${mode.key}.png")
    }

    @Test fun mobileLight() = shoot(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun mobileDark() = shoot(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun desktopLight() = shoot(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun desktopDark() = shoot(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun materialLight() = shoot(DesignStyle.Material, ColorMode.Light)

    @Test fun materialDark() = shoot(DesignStyle.Material, ColorMode.Dark)
}
