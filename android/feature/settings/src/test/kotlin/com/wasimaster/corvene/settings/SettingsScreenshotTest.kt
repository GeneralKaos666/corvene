package com.wasimaster.corvene.settings

import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Settings' screens in every style, light and dark, on a phone: the list, Prompts, Integrations, Flags, Repository settings. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h740dp-xxhdpi")
class SettingsScreenshotTest(private val style: DesignStyle, private val mode: ColorMode) {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(name: String, content: @Composable () -> Unit) {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) { content() }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/${name}_${style.key}_${mode.key}.png")
    }

    @Test
    fun list() = shoot("settings_list") { SettingsListScreen(onOpen = {}) }

    @Test
    fun prompts() = shoot("settings_prompts") { PromptsScreen(SampleSettings, { _, _ -> }) }

    @Test
    fun integrations() = shoot("settings_integrations") {
        IntegrationsScreen(
            SampleSettings.copy(externalEditor = "Acode"),
            listOf(EditorApp("Acode", "com.foxdebug.acode/.MainActivity"), EditorApp("Markor", "net.gsantner.markor/.Main")),
            termuxInstalled = true,
            writer = { _, _ -> },
        )
    }

    @Test
    fun flags() = shoot("flags") {
        val pending = SampleFlags.copy(flags = SampleFlags.flags.map { if (it.slug == "design-style") it.copy(restartPending = true) else it })
        FlagsScreen(pending, FlagsView(), {}, mapOf("commit-template" to "not a template"), NoFlagActions)
    }

    @Test
    fun repositorySettings() {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) {
                RepositorySettingsDialog(SampleRepositorySettings, RepositorySettingsTab.Remote, onSave = {}, onDismissRequest = {})
            }
        }
        compose.waitForIdle()
        captureScreenRoboImage("src/test/screenshots/repository_settings_${style.key}_${mode.key}.png")
    }

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}_{1}")
        fun matrix(): List<Array<Any>> = DesignStyle.entries.flatMap { style ->
            listOf(ColorMode.Light, ColorMode.Dark).map { arrayOf<Any>(style, it) }
        }
    }
}

internal object NoFlagActions : FlagActions {
    override fun set(slug: String, value: String) = Unit

    override fun reset(slug: String) = Unit

    override fun applyPreset(slug: String) = Unit

    override fun resetAll() = Unit

    override fun relaunch() = Unit
}
