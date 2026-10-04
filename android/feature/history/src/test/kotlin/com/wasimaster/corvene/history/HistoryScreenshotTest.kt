package com.wasimaster.corvene.history

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

/** History and the commit detail in every style, light and dark, on a phone; both side by side on a tablet. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class HistoryScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(style: DesignStyle, mode: ColorMode, name: String, content: @Composable () -> Unit) {
        compose.setContent { CorveneTheme(style, mode, highContrast = false, dynamicColor = false, content = content) }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png")
    }

    @Composable
    private fun History(modifier: Modifier = Modifier) = HistoryScreen(
        SampleHistory,
        remember { samplePager() },
        null,
        listOf("main", "feature-a"),
        "main",
        true,
        NoHistoryActions,
        modifier,
        now = SAMPLE_NOW,
    )

    private fun history(style: DesignStyle, mode: ColorMode) = shoot(style, mode, "history_${style.key}_${mode.key}") { History() }

    private fun detail(style: DesignStyle, mode: ColorMode) = shoot(style, mode, "commit_detail_${style.key}_${mode.key}") {
        CommitDetailScreen(SampleDetail, NoDetailActions, now = SAMPLE_NOW)
    }

    @Test fun historyMobileLight() = history(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun historyMobileDark() = history(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun historyDesktopLight() = history(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun historyDesktopDark() = history(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun historyMaterialLight() = history(DesignStyle.Material, ColorMode.Light)

    @Test fun historyMaterialDark() = history(DesignStyle.Material, ColorMode.Dark)

    @Test fun detailMobileLight() = detail(DesignStyle.GitHubMobile, ColorMode.Light)

    @Test fun detailMobileDark() = detail(DesignStyle.GitHubMobile, ColorMode.Dark)

    @Test fun detailDesktopLight() = detail(DesignStyle.GitHubDesktop, ColorMode.Light)

    @Test fun detailDesktopDark() = detail(DesignStyle.GitHubDesktop, ColorMode.Dark)

    @Test fun detailMaterialLight() = detail(DesignStyle.Material, ColorMode.Light)

    @Test fun detailMaterialDark() = detail(DesignStyle.Material, ColorMode.Dark)

    @Test
    fun comparing() = shoot(DesignStyle.GitHubMobile, ColorMode.Dark, "history_compare_github-mobile_dark") {
        HistoryScreen(
            SampleHistory,
            remember { samplePager() },
            Comparison("feature-a", ahead = false),
            listOf("main", "feature-a"),
            "main",
            true,
            NoHistoryActions,
            now = SAMPLE_NOW,
        )
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-mdpi")
    fun expandedHistoryAndCommit() = shoot(DesignStyle.GitHubDesktop, ColorMode.Light, "history_expanded_github-desktop") {
        Row(Modifier.fillMaxSize()) {
            History(Modifier.weight(0.35f))
            VerticalDivider()
            CommitDetailScreen(SampleDetail, NoDetailActions, Modifier.weight(0.65f), now = SAMPLE_NOW)
        }
    }
}
