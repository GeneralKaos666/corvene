package com.wasimaster.corvene.branches

import org.junit.Assert.assertEquals
import org.junit.Test

class BranchGroupsTest {

    @Test
    fun `default, recent without default, other by name, remotes without a local twin`() {
        val groups = groupBranches(SampleBranches, "")
        assertEquals(listOf("main"), groups.default.map { it.name })
        assertEquals(listOf("feature-a"), groups.recent.map { it.name })
        assertEquals(listOf("feature-b", "wasi/very-long-branch-name-for-the-chip"), groups.other.map { it.name })
        assertEquals(listOf("origin/release"), groups.remote.map { it.name })
    }

    @Test
    fun `the filter matches anywhere, ignoring case`() {
        val groups = groupBranches(SampleBranches, "LONG")
        assertEquals(listOf("wasi/very-long-branch-name-for-the-chip"), (groups.default + groups.recent + groups.other).map { it.name })
        assertEquals(true, groups.remote.isEmpty())
    }

    @Test
    fun `names are sanitized as git takes them`() {
        assertEquals("my-topic", sanitizeBranchName("  my topic "))
        assertEquals("a-b", sanitizeBranchName("a..b"))
        assertEquals("fix-it", sanitizeBranchName("fix~it"))
        assertEquals("x", sanitizeBranchName("-x.lock"))
    }
}
