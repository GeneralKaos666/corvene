package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The shared components in the three styles, light and dark. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w400dp-h1200dp-xhdpi")
class ComponentsScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(mode: ColorMode, name: String) {
        compose.setContent {
            DesignStyleSamples(Modifier.width(400.dp), colorMode = mode) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    PrimerButton("Commit", {}, variant = PrimerButtonVariant.Primary, leadingIcon = Octicons.GitCommit)
                    PrimerButton("Fetch", {}, leadingIcon = Octicons.Sync)
                    PrimerButton("Discard", {}, variant = PrimerButtonVariant.Danger)
                }
                Column {
                    ActionListGroupHeader("Recent")
                    ActionListItem(
                        "corvene",
                        description = "main",
                        selected = true,
                        leading = { IconTile(Octicons.Repo, CorveneTheme.colors.accent) },
                        trailing = { CounterLabel("↑2") },
                    )
                    ActionListDivider()
                    ActionListItem("notes", leading = { IconTile(Octicons.DeviceDesktop, CorveneTheme.colors.done) })
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Avatar("wasi-master")
                    PrimerIconButton(Octicons.Search, "Search", {}, tint = OcticonTint.Link)
                    PrimerIconButton(Octicons.Trash, "Remove", {}, tint = OcticonTint.Danger)
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/components_$name.png")
    }

    @Test
    fun light() = shoot(ColorMode.Light, "light")

    @Test
    fun dark() = shoot(ColorMode.Dark, "dark")

    @Test
    fun `palettes differ per style and contrast`() {
        assertNotEquals(githubPalette(DesignStyle.GitHubMobile, false, false), githubPalette(DesignStyle.GitHubDesktop, false, false))
        assertNotEquals(githubPalette(DesignStyle.GitHubMobile, true, false), githubPalette(DesignStyle.GitHubMobile, true, true))
        assertEquals(GitHubDesktopHighContrast, githubPalette(DesignStyle.GitHubDesktop, true, true))
    }
}
