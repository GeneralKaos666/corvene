package com.wasimaster.corvene.changes

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
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

/**
 * The Changes list and the diff in every style, light and dark, on a phone
 * (`recordRoborazziFossDebug` writes src/test/screenshots/), plus the two
 * panes side by side on a tablet.
 */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class ChangesScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(style: DesignStyle, mode: ColorMode, name: String, content: @Composable () -> Unit) {
        compose.setContent { CorveneTheme(style, mode, highContrast = false, dynamicColor = false, content = content) }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png")
    }

    private fun changes(style: DesignStyle, mode: ColorMode) =
        shoot(style, mode, "changes_${style.key}_${mode.key}") { ChangesScreen(SampleChanges, true, NoChangesActions) }

    private fun diff(style: DesignStyle, mode: ColorMode) = shoot(style, mode, "diff_${style.key}_${mode.key}") {
        DiffScreen(SampleHeader, remember { samplePager() }, remember { DiffLineCache() }, false, NoDiffActions)
    }

    @Test fun changesMobileLight() = changes(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun changesMobileDark() = changes(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun changesDesktopLight() = changes(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun changesDesktopDark() = changes(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun changesMaterialLight() = changes(DesignStyle.Material, ColorMode.Light)

    @Test fun changesMaterialDark() = changes(DesignStyle.Material, ColorMode.Dark)

    @Test fun diffMobileLight() = diff(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun diffMobileDark() = diff(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun diffDesktopLight() = diff(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun diffDesktopDark() = diff(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun diffMaterialLight() = diff(DesignStyle.Material, ColorMode.Light)

    @Test fun diffMaterialDark() = diff(DesignStyle.Material, ColorMode.Dark)

    @Test
    fun noChangesMobile() = shoot(DesignStyle.GitHubMobile, ColorMode.Light, "changes_empty_github-mobile") {
        ChangesScreen(EmptyChanges, true, NoChangesActions)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-mdpi")
    fun expandedListAndDiff() = shoot(DesignStyle.GitHubDesktop, ColorMode.Light, "changes_expanded_github-desktop") {
        Row(Modifier.fillMaxSize()) {
            ChangesScreen(SampleChanges, true, NoChangesActions, Modifier.weight(0.35f))
            VerticalDivider()
            DiffScreen(SampleHeader, remember { samplePager() }, remember { DiffLineCache() }, false, NoDiffActions, Modifier.weight(0.65f))
        }
    }
}
