package com.wasimaster.corvene.repositories

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.CloneableRepositoryVm
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The Clone and Create forms: what their controls send, and the URL helpers. */
@RunWith(AndroidJUnit4::class)
class RepositoryFormsTest {

    @get:Rule
    val compose = createComposeRule()

    private val calls = mutableListOf<String>()

    private val actions = object : CloneActions {
        override fun tab(tab: CloneTab) {
            calls += "tab $tab"
        }

        override fun url(url: String) {
            calls += "url $url"
        }

        override fun pick(repository: CloneableRepositoryVm) {
            calls += "pick ${repository.name}"
        }

        override fun filter(text: String) {
            calls += "filter $text"
        }

        override fun choosePath() {
            calls += "choose"
        }

        override fun shallow(on: Boolean) {
            calls += "shallow $on"
        }

        override fun signIn() {
            calls += "signIn"
        }

        override fun clone() {
            calls += "clone"
        }

        override fun close() {
            calls += "close"
        }
    }

    private fun show(state: CloneState) {
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                CloneScreen(state, actions)
            }
        }
    }

    @Test
    fun `the url tab sends what is typed, the path, the shallow switch and clone`() {
        show(CloneState(CloneTab.Url, url = "", path = "/files/repositories", shallow = false, signedIn = false))
        compose.onNodeWithTag(TAG_CONFIRM).assertIsNotEnabled()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_CLONE_URL))).performTextInput("octocat/Hello-World")
        compose.onNodeWithTag(TAG_LOCAL_PATH).performClick()
        compose.onNodeWithTag(TAG_CLONE_SHALLOW).performClick()
        assertEquals(listOf("url octocat/Hello-World", "choose", "shallow true"), calls)
    }

    @Test
    fun `clone is enabled with a url and a path`() {
        show(CloneState(CloneTab.Url, url = "https://github.com/octocat/Hello-World.git", path = "/r/Hello-World", shallow = false, signedIn = true))
        compose.onNodeWithTag(TAG_CONFIRM).assertIsEnabled().performClick()
        assertEquals(listOf("clone"), calls)
    }

    @Test
    fun `signed out, the github tab asks to sign in`() {
        show(CloneState(CloneTab.GitHub, url = "", path = "/r", shallow = false, signedIn = false))
        compose.onNodeWithTag(TAG_CLONE_SIGN_IN).performClick()
        compose.onNodeWithText("Clone from a URL").performClick()
        assertEquals(listOf("signIn", "tab Url"), calls)
    }

    @Test
    fun `signed in, the github tab lists and picks`() {
        show(CloneState(CloneTab.GitHub, url = "", path = "/r", shallow = false, signedIn = true, repositories = SampleCloneable))
        compose.onNodeWithText("octocat/Spoon-Knife").performClick()
        assertEquals(listOf("pick Spoon-Knife"), calls)
    }

    @Test
    fun `create sends the name and needs one`() {
        var form = CreateForm()
        var created = 0
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                CreateRepositoryScreen(form, "/r", onChange = { form = it }, onChooseBase = {}, onCreate = { created++ }, onClose = {})
            }
        }
        compose.onNodeWithTag(TAG_CONFIRM).assertIsNotEnabled()
        compose.onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_CREATE_NAME))).performTextInput("my project")
        assertEquals("my project", form.name)
    }

    @Test
    fun `urls and names`() {
        assertEquals("https://github.com/octocat/Hello-World", cloneUrlFor(" octocat/Hello-World "))
        assertEquals("git@github.com:a/b.git", cloneUrlFor("git@github.com:a/b.git"))
        assertEquals("Hello-World", repositoryNameOf("https://github.com/octocat/Hello-World.git"))
        assertEquals("b", repositoryNameOf("git@github.com:a/b.git"))
        assertEquals("Spoon-Knife", repositoryNameOf("https://github.com/octocat/Spoon-Knife/"))
        assertEquals("my-project", sanitizedName("my project"))
    }
}
