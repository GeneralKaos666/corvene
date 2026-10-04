package com.wasimaster.corvene

import org.junit.Assert.assertEquals
import org.junit.Test

class BannerFieldsTest {

    @Test
    fun `strings, options and numbers from the Debug form`() {
        assertEquals(
            mapOf("our_branch" to "main", "their_branch" to "feature-a"),
            bannerFields("""SuccessfulMerge { our_branch: "main", their_branch: Some("feature-a") }"""),
        )
        assertEquals(
            mapOf("repo" to "3", "target_branch" to "release \"x\"", "count" to "2"),
            bannerFields("""SuccessfulCherryPick { repo: 3, target_branch: "release \"x\"", count: 2 }"""),
        )
        assertEquals(
            mapOf("base_branch" to "main"),
            bannerFields("""SuccessfulRebase { target_branch: None, base_branch: Some("main") }""").filterKeys { it == "base_branch" },
        )
    }
}
