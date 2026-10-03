package com.wasimaster.corvene.repositories

import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm
import org.junit.Assert.assertEquals
import org.junit.Test

class RepositoryGroupsTest {

    private fun repo(id: Int, name: String, github: String? = null) =
        RepoVm(id.toULong(), name, "/r/$name", github, false, null, 0u, null, null)

    @Test
    fun `recent first, then owners, then other`() {
        val list = RepoListVm(
            selected = null,
            recent = listOf(3u, 1u),
            repositories = listOf(repo(1, "zeta", "b/zeta"), repo(2, "Alpha", "a/alpha"), repo(3, "local"), repo(4, "beta", "b/beta")),
            signedIn = false,
            welcomeCompleted = true,
        )
        val groups = groupRepositories(list)
        assertEquals(
            listOf(RepositoryGroup.Kind.Recent, RepositoryGroup.Kind.Owner, RepositoryGroup.Kind.Owner, RepositoryGroup.Kind.Other),
            groups.map { it.kind },
        )
        assertEquals(listOf("local", "zeta"), groups[0].repositories.map { it.name })
        assertEquals("a", groups[1].title)
        assertEquals(listOf("beta", "zeta"), groups[2].repositories.map { it.name })
        assertEquals(listOf("local"), groups[3].repositories.map { it.name })
    }

    @Test
    fun `a single repository has no recent group`() {
        val list = RepoListVm(null, listOf(1u), listOf(repo(1, "only")), signedIn = false, welcomeCompleted = true)
        assertEquals(listOf(RepositoryGroup.Kind.Other), groupRepositories(list).map { it.kind })
    }

    @Test
    fun `recent keeps three`() {
        val repos = (1..5).map { repo(it, "r$it") }
        val list = RepoListVm(null, (1..5).map { it.toULong() }, repos, signedIn = false, welcomeCompleted = true)
        assertEquals(3, groupRepositories(list).first().repositories.size)
    }
}
