package com.wasimaster.corvene.onboarding

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.SignInStepVm
import com.wasimaster.corvene.ffi.gen.SignInVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The Welcome steps: what each button sends. */
@RunWith(AndroidJUnit4::class)
class WelcomeScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val calls = mutableListOf<String>()
    private val steps = mutableListOf<WelcomeStep>()
    private val finished = mutableListOf<Pair<String?, String?>>()

    private val actions = object : SignInActions {
        override fun signIn() {
            calls += "signIn"
        }

        override fun signInEnterprise(host: String, token: String) {
            calls += "enterprise $host $token"
        }

        override fun cancel() {
            calls += "cancel"
        }

        override fun openUrl(url: String) {
            calls += "open $url"
        }

        override fun copy(text: String) {
            calls += "copy $text"
        }
    }

    private fun show(state: WelcomeState, name: String = "") {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                WelcomeScreen(state, actions, onStep = { steps += it }, onFinish = { n, e -> finished += n to e }, initialName = name)
            }
        }
    }

    @Test
    fun `start goes to sign in`() {
        show(WelcomeState(WelcomeStep.Start, null, null))
        compose.onNodeWithTag(TAG_GET_STARTED).performClick()
        assertEquals(listOf(WelcomeStep.SignIn), steps)
    }

    @Test
    fun `sign in starts the browser flow or skips`() {
        show(WelcomeState(WelcomeStep.SignIn, null, null))
        compose.onNodeWithTag(TAG_SIGN_IN_BROWSER).performClick()
        compose.onNodeWithTag(TAG_SKIP).performClick()
        assertEquals(listOf("signIn"), calls)
        assertEquals(listOf(WelcomeStep.ConfigureGit), steps)
    }

    @Test
    fun `the enterprise form sends host and token`() {
        show(WelcomeState(WelcomeStep.SignIn, null, null))
        compose.onNodeWithTag(TAG_SIGN_IN_ENTERPRISE).performClick()
        compose.onNodeWithTag(TAG_ENTERPRISE_SUBMIT).assertIsNotEnabled()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_ENTERPRISE_HOST))).performTextInput("github.example.com")
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_ENTERPRISE_TOKEN))).performTextInput("ghp_test")
        compose.onNodeWithTag(TAG_ENTERPRISE_SUBMIT).performClick()
        assertEquals(listOf("enterprise github.example.com ghp_test"), calls)
    }

    @Test
    fun `the device code is shown, copied and opened`() {
        show(WelcomeState(WelcomeStep.SignIn, SampleDeviceCode, null))
        compose.onNodeWithText("WDJB-MJHT").assertExists()
        compose.onNodeWithTag(TAG_COPY_CODE).performClick()
        compose.onNodeWithTag(TAG_OPEN_GITHUB).performClick()
        compose.onNodeWithText("Cancel").performScrollTo().performClick()
        assertEquals(listOf("copy WDJB-MJHT", "open https://github.com/login/device", "cancel"), calls)
    }

    @Test
    fun `an error offers to try again`() {
        show(WelcomeState(WelcomeStep.SignIn, SignInVm("https://api.github.com", SignInStepVm.Error("timed out")), null))
        compose.onNodeWithText("timed out").assertExists()
        compose.onNodeWithTag(TAG_TRY_AGAIN).performClick()
        assertEquals(listOf("signIn"), calls)
    }

    @Test
    fun `signed in continues`() {
        show(WelcomeState(WelcomeStep.SignIn, null, SampleAccount))
        compose.onNodeWithTag(TAG_SIGNED_IN).assertExists()
        compose.onNodeWithTag(TAG_CONTINUE).performClick()
        assertEquals(listOf(WelcomeStep.ConfigureGit), steps)
    }

    @Test
    fun `configure git needs a name and an email, skip writes nothing`() {
        show(WelcomeState(WelcomeStep.ConfigureGit, null, null), name = "Mona")
        compose.onNodeWithTag(TAG_FINISH).assertIsNotEnabled()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_EMAIL))).performTextInput("mona@example.com")
        compose.onNodeWithTag(TAG_FINISH).assertIsEnabled().performClick()
        compose.onNodeWithTag(TAG_SKIP).performClick()
        assertEquals(listOf<Pair<String?, String?>>("Mona" to "mona@example.com", null to null), finished)
    }
}
