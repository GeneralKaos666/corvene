package com.wasimaster.corvene.settings

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.AccountVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

private val Octocat = AccountVm(endpoint = "https://api.github.com", login = "octocat", name = "The Octocat", avatarUrl = null, avatarPath = null, host = "github.com", emails = emptyList())

/** Settings › Accounts: sign out asks first; sign-in rows by what is missing. */
@RunWith(AndroidJUnit4::class)
class AccountsScreenTest {

    @get:Rule
    val compose = createComposeRule()

    private val signIns = mutableListOf<Boolean>()
    private val signOuts = mutableListOf<String>()

    private fun show(accounts: List<AccountVm>) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                AccountsScreen(accounts, emptyMap(), onSignIn = { signIns += it }, onSignOut = { signOuts += it.endpoint })
            }
        }
    }

    @Test
    fun `sign out asks first`() {
        show(listOf(Octocat))
        compose.onNodeWithTag("${TAG_SIGN_OUT}octocat").performClick()
        compose.onNodeWithTag(TAG_SIGN_OUT_CONFIRM).performClick()
        assertEquals(listOf("https://api.github.com"), signOuts)
        compose.onNodeWithTag(TAG_SIGN_IN).assertDoesNotExist()
    }

    @Test
    fun `signed out offers both sign-ins`() {
        show(emptyList())
        compose.onNodeWithTag(TAG_SIGN_IN).performClick()
        compose.onNodeWithTag(TAG_SIGN_IN_ENTERPRISE).performClick()
        assertEquals(listOf(false, true), signIns)
    }
}
