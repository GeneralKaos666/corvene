package com.wasimaster.corvene.platform

import android.content.Intent
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Intents that open the app → what `appUrl` gets. */
@RunWith(AndroidJUnit4::class)
class AppLinksTest {

    @Test
    fun `app links pass through`() {
        assertEquals(
            "x-corvene://openRepo/octocat/Spoon-Knife",
            AppLinks.appUrl(Intent.ACTION_VIEW, "x-corvene://openRepo/octocat/Spoon-Knife", null),
        )
        assertEquals(
            "x-corvene-auth://oauth?code=abc&state=xyz",
            AppLinks.appUrl(Intent.ACTION_VIEW, "x-corvene-auth://oauth?code=abc&state=xyz", null),
        )
    }

    @Test
    fun `a viewed github link offers to clone`() {
        assertEquals(
            "x-corvene://openRepo/https://github.com/octocat/Hello-World",
            AppLinks.appUrl(Intent.ACTION_VIEW, "https://github.com/octocat/Hello-World", null),
        )
    }

    @Test
    fun `shared text with an address offers to clone it`() {
        assertEquals(
            "x-corvene://openRepo/https://github.com/octocat/Spoon-Knife",
            AppLinks.appUrl(Intent.ACTION_SEND, null, "Look at this\nhttps://github.com/octocat/Spoon-Knife please"),
        )
        assertEquals(
            "x-corvene://openRepo/git@github.com:octocat/Hello-World.git",
            AppLinks.appUrl(Intent.ACTION_SEND, null, "git@github.com:octocat/Hello-World.git"),
        )
    }

    @Test
    fun `other text and other actions ask nothing`() {
        assertNull(AppLinks.appUrl(Intent.ACTION_SEND, null, "just some words"))
        assertNull(AppLinks.appUrl(Intent.ACTION_MAIN, null, null))
        assertNull(AppLinks.appUrl(Intent.ACTION_VIEW, "mailto:someone@example.com", null))
    }

    @Test
    fun `the full intent is read`() {
        val intent = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, "https://github.com/a/b")
        assertEquals("x-corvene://openRepo/https://github.com/a/b", AppLinks.appUrl(intent))
    }

    @Test
    fun `sign-in pages open in a custom tab`() {
        assertTrue(isSignInUrl("https://github.com/login/oauth/authorize?client_id=x"))
        assertTrue(isSignInUrl("https://github.com/login/device"))
        assertFalse(isSignInUrl("https://github.com/octocat/Hello-World/commit/abc"))
    }
}
