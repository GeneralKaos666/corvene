package com.wasimaster.corvene.onboarding

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
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The Welcome steps and the device code, in every style, light and dark, on a phone. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xxhdpi")
class WelcomeScreenshotTest(private val style: DesignStyle, private val mode: ColorMode) {

    @get:Rule
    val compose = createComposeRule()

    private fun shoot(name: String, state: WelcomeState, initialName: String = "") {
        compose.setContent {
            CorveneTheme(style, mode, highContrast = false, dynamicColor = false) {
                WelcomeScreen(state, NoSignInActions, onStep = {}, onFinish = { _, _ -> }, initialName = initialName)
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/welcome_${name}_${style.key}_${mode.key}.png")
    }

    @Test
    fun start() = shoot("start", WelcomeState(WelcomeStep.Start, null, null))

    @Test
    fun signIn() = shoot("sign_in", WelcomeState(WelcomeStep.SignIn, null, null))

    @Test
    fun deviceCode() = shoot("device_code", WelcomeState(WelcomeStep.SignIn, SampleDeviceCode, null))

    @Test
    fun configureGit() = shoot("configure_git", WelcomeState(WelcomeStep.ConfigureGit, null, SampleAccount), "The Octocat")

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0} {1}")
        fun parameters(): List<Array<Any>> =
            DesignStyle.entries.flatMap { style -> listOf(ColorMode.Light, ColorMode.Dark).map { arrayOf<Any>(style, it) } }
    }
}
