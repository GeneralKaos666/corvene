package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.state.ToggleableState
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

/** The shared components in the three styles, light and dark, one page per family. */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w400dp-h2000dp-xhdpi")
class ComponentsScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(mode: ColorMode, name: String, content: @Composable () -> Unit) {
        compose.setContent { DesignStyleSamples(Modifier.width(400.dp), colorMode = mode, content = content) }
        compose.onRoot().captureRoboImage("src/test/screenshots/components_${name}_${mode.key}.png")
    }

    private val basics: @Composable () -> Unit = {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PrimerButton("Commit", {}, variant = PrimerButtonVariant.Primary, leadingIcon = Octicons.GitCommit)
            PrimerButton("Fetch", {}, leadingIcon = Octicons.Sync)
            PrimerButton("Discard", {}, variant = PrimerButtonVariant.Danger)
        }
        ActionList {
            ActionListGroupHeader("Recent")
            ActionListItem(
                "corvene",
                description = "main",
                selected = true,
                leading = { IconTile(Octicons.Repo, CorveneTheme.colors.accent) },
                trailing = { CounterLabel("↑2") },
            )
            ActionListDivider()
            ActionListItem("notes", leading = { IconTile(Octicons.DeviceDesktop, CorveneTheme.colors.done) }, count = 4, chevron = true)
            ActionListItem("main", checked = true, leading = { Octicon(Octicons.GitBranch, null) })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar("wasi-master")
            PrimerIconButton(Octicons.Search, "Search", {}, tint = OcticonTint.Link)
            PrimerIconButton(Octicons.Trash, "Remove", {}, tint = OcticonTint.Danger)
            Label("Partial", variant = LabelVariant.Accent)
            StateLabel("Conflicts", StateLabelState.Closed, icon = Octicons.Alert)
            BranchName("main", icon = true)
        }
    }

    private val inputs: @Composable () -> Unit = {
        PrimerTextField("Fix the diff gutter", {}, label = "Summary")
        FilterField("", {}, placeholder = "Filter")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            PrimerCheckbox(ToggleableState.On, {})
            PrimerCheckbox(ToggleableState.Indeterminate, {})
            PrimerCheckbox(ToggleableState.Off, {})
            PrimerSwitch(checked = true, onCheckedChange = {})
            SegmentedControl(listOf("Unified", "Split"), 0, {})
        }
        RadioRow("System", selected = true, onClick = {})
        SwitchRow("Hide whitespace", checked = false, onCheckedChange = {})
    }

    private val feedback: @Composable () -> Unit = {
        Flash(
            "You have stashed changes on this branch.",
            icon = Octicons.Stack,
            action = { PrimerButton("Restore", {}, variant = PrimerButtonVariant.Link) },
        )
        Flash("Couldn't load the diff", variant = FlashVariant.Danger, onDismiss = {})
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Spinner(size = SpinnerSize.Small)
            ProgressBar(0.4f, Modifier.weight(1f))
        }
        Blankslate(Octicons.CheckCircle, "No local changes", description = "Here are some friendly suggestions.")
        Column {
            Truncate("crates/corvene-ffi/src/vm/very/long/path/to/the/diff.rs", Modifier.width(200.dp))
            FilePath("android/feature/changes/ChangesScreen.kt", oldPath = "ChangesList.kt")
        }
    }

    private val navigation: @Composable () -> Unit = {
        RepositoryTopChrome(
            repository = "corvene",
            owner = "wasi-master",
            branch = "main",
            sync = SyncButtonModel("Fetch origin", "Never fetched", Octicons.Sync, enabled = false),
            onRepositoryClick = {},
            onBranchClick = {},
            onSync = {},
            labels = PreviewChromeLabels,
            tabs = { UnderlineNav(listOf(UnderlineNavItem("Changes", 2), UnderlineNavItem("History")), 0, {}) },
        )
        ChipRow {
            PrimerChip("Included", selected = true, onClick = {})
            PrimerChip("New", selected = false, onClick = {})
        }
        PrimerTopAppBar("Appearance", subtitle = "Settings", onBack = {})
    }

    private val overlays: @Composable () -> Unit = {
        SelectPanelContent(SelectPanelParts("Repositories", {}, "", {}, "Filter", "Close", { PrimerButton("Add repository", {}) })) {
            item { ActionListGroupHeader("Recent") }
            item { ActionListItem("corvene", checked = true) }
            item { ActionListItem("desktop", checked = false) }
        }
        PrimerDialogSurface(
            "Discard changes?",
            {},
            Modifier,
            fullScreen = false,
            closeDescription = "Close",
            confirmButton = { PrimerButton("Discard changes", {}, variant = PrimerButtonVariant.Danger) },
            dismissButton = { PrimerButton("Cancel", {}) },
        ) { Text("README.md will be moved to the trash.") }
    }

    @Test fun basicsLight() = shoot(ColorMode.Light, "basics", basics)

    @Test fun basicsDark() = shoot(ColorMode.Dark, "basics", basics)

    @Test fun inputsLight() = shoot(ColorMode.Light, "inputs", inputs)

    @Test fun inputsDark() = shoot(ColorMode.Dark, "inputs", inputs)

    @Test fun feedbackLight() = shoot(ColorMode.Light, "feedback", feedback)

    @Test fun feedbackDark() = shoot(ColorMode.Dark, "feedback", feedback)

    @Test fun navigationLight() = shoot(ColorMode.Light, "navigation", navigation)

    @Test fun navigationDark() = shoot(ColorMode.Dark, "navigation", navigation)

    @Test fun overlaysLight() = shoot(ColorMode.Light, "overlays", overlays)

    @Test fun overlaysDark() = shoot(ColorMode.Dark, "overlays", overlays)

    @Test
    fun `palettes differ per style and contrast`() {
        assertNotEquals(githubPalette(DesignStyle.GitHubMobile, false, false), githubPalette(DesignStyle.GitHubDesktop, false, false))
        assertNotEquals(githubPalette(DesignStyle.GitHubMobile, true, false), githubPalette(DesignStyle.GitHubMobile, true, true))
        assertEquals(GitHubDesktopHighContrast, githubPalette(DesignStyle.GitHubDesktop, true, true))
        assertNotEquals(diffPaletteOf(DesignStyle.GitHubMobile, false), diffPaletteOf(DesignStyle.Material, false))
    }
}
