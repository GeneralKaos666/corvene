package com.wasimaster.corvene.branches

import androidx.compose.runtime.Composable
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

/** The branch panel (drawn in place) in every style, light and dark, on a phone. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class BranchesScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(style: DesignStyle, mode: ColorMode, name: String, content: @Composable () -> Unit) {
        compose.setContent { CorveneTheme(style, mode, highContrast = false, dynamicColor = false, content = content) }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png")
    }

    private fun sheet(style: DesignStyle, mode: ColorMode) = shoot(style, mode, "branch_sheet_${style.key}_${mode.key}") {
        BranchSheet(SampleBranches, SamplePulls, NoBranchActions, {}, inline = true, now = SAMPLE_NOW)
    }

    @Test fun sheetMobileLight() = sheet(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun sheetMobileDark() = sheet(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun sheetDesktopLight() = sheet(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun sheetDesktopDark() = sheet(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun sheetMaterialLight() = sheet(DesignStyle.Material, ColorMode.Light)

    @Test fun sheetMaterialDark() = sheet(DesignStyle.Material, ColorMode.Dark)
}
