package com.wasimaster.corvene.repositories

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.RepoListVm
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The repository list in every design style, light and dark, on a phone-sized
 * screen. `recordRoborazziFossDebug` writes src/test/screenshots/,
 * `verifyRoborazziFossDebug` compares against it.
 */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class RepositoryListScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(style: DesignStyle, mode: ColorMode, list: RepoListVm, name: String) {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) {
                RepositoryListScreen(list, refreshing = false, onSelect = {}, onRemove = {}, onAdd = {}, onRefresh = {})
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/repository_list_$name.png")
    }

    @Test
    fun githubMobileLight() = shoot(DesignStyle.GitHubMobile, ColorMode.Light, SampleList, "github_mobile_light")

    @Test
    fun githubMobileDark() = shoot(DesignStyle.GitHubMobile, ColorMode.Dark, SampleList, "github_mobile_dark")

    @Test
    fun githubDesktopLight() = shoot(DesignStyle.GitHubDesktop, ColorMode.Light, SampleList, "github_desktop_light")

    @Test
    fun githubDesktopDark() = shoot(DesignStyle.GitHubDesktop, ColorMode.Dark, SampleList, "github_desktop_dark")

    @Test
    fun materialLight() = shoot(DesignStyle.Material, ColorMode.Light, SampleList, "material_light")

    @Test
    fun materialDark() = shoot(DesignStyle.Material, ColorMode.Dark, SampleList, "material_dark")

    @Test
    fun emptyGithubMobile() = shoot(DesignStyle.GitHubMobile, ColorMode.Light, EmptyList, "empty_github_mobile")
}
