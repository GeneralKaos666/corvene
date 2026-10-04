package com.wasimaster.corvene.repositories

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.captureRoboImage
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Clone (both tabs), Create and Add in every style, light and dark, on a phone. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class RepositoryFormsScreenshotTest(private val style: DesignStyle, private val mode: ColorMode) {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(name: String, content: @androidx.compose.runtime.Composable () -> Unit) {
        compose.setContent { CorveneTheme(style, mode, highContrast = false, dynamicColor = false) { content() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/${name}_${style.key}_${mode.key}.png")
    }

    @Test
    fun cloneUrl() = shoot("clone_url") {
        CloneScreen(
            CloneState(
                CloneTab.Url,
                url = "octocat/Spoon-Knife",
                path = "/data/user/0/com.wasimaster.corvene/files/repositories/Spoon-Knife",
                shallow = false,
                signedIn = false,
            ),
            NoCloneActions,
        )
    }

    @Test
    fun cloneGitHub() = shoot("clone_github") {
        CloneScreen(
            CloneState(
                CloneTab.GitHub,
                url = SampleCloneable[0].cloneUrl,
                path = "/data/user/0/com.wasimaster.corvene/files/repositories/Hello-World",
                shallow = true,
                signedIn = true,
                repositories = SampleCloneable,
            ),
            NoCloneActions,
        )
    }

    @Test
    fun create() = shoot("create") {
        CreateRepositoryScreen(
            CreateForm(name = "notes", description = "Things to remember", gitignore = "Kotlin", license = "MIT License"),
            "/data/user/0/com.wasimaster.corvene/files/repositories",
            onChange = {},
            onChooseBase = {},
            onCreate = {},
            onClose = {},
        )
    }

    @Test
    fun add() = shoot("add") {
        AddRepositoryScreen("/storage/emulated/0/Corvene/demo2", onChoose = {}, onAdd = {}, onClose = {}, onAllFilesAccess = {})
    }

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0} {1}")
        fun parameters(): List<Array<Any>> =
            DesignStyle.entries.flatMap { style -> listOf(ColorMode.Light, ColorMode.Dark).map { arrayOf<Any>(style, it) } }
    }
}
