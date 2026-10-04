package com.wasimaster.corvene.settings

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

/** Settings on a 1280×800 tablet: the sections list beside Prompts. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-mdpi")
class SettingsExpandedScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun expanded() {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubDesktop, ColorMode.Light, highContrast = false, dynamicColor = false) {
                SettingsPanes(selected = SettingsSection.Prompts, onSelect = {}) { PromptsScreen(SampleSettings, { _, _ -> }) }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/settings_expanded_github-desktop_light.png")
    }
}
