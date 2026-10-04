package com.wasimaster.corvene.repositories

import com.wasimaster.corvene.ffi.gen.RepoGroupVm
import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm
import org.junit.Assert.assertEquals
import org.junit.Test

class RepositoryGroupsTest {

    private fun repo(id: Int, name: String, github: String? = null) =
        RepoVm(id.toULong(), name, "/r/$name", github, github?.substringBefore('/'), false, false, null, false, null, 0u, null, null)

    private val list = RepoListVm(
        selected = null,
        recent = listOf(3u, 1u),
        repositories = listOf(repo(1, "zeta", "b/zeta"), repo(2, "Alpha", "a/alpha"), repo(3, "local"), repo(4, "beta", "b/beta")),
        groups = listOf(
            RepoGroupVm("Recent", listOf(3u, 1u)),
            RepoGroupVm("a", listOf(2u)),
            RepoGroupVm("b", listOf(4u, 1u)),
            RepoGroupVm("Other", listOf(3u)),
        ),
        signedIn = false,
        welcomeCompleted = true,
    )

    @Test
    fun `the engine's groups resolve to rows in order`() {
        val groups = groupRepositories(list)
        assertEquals(
            listOf(RepositoryGroup.Kind.Recent, RepositoryGroup.Kind.Owner, RepositoryGroup.Kind.Owner, RepositoryGroup.Kind.Other),
            groups.map { it.kind },
        )
        assertEquals(listOf("local", "zeta"), groups[0].repositories.map { it.name })
        assertEquals("a", groups[1].title)
        assertEquals(listOf("beta", "zeta"), groups[2].repositories.map { it.name })
    }

    @Test
    fun `a filter drops recent and empty groups`() {
        val groups = groupRepositories(list, "ZE")
        assertEquals(listOf(RepositoryGroup.Kind.Owner), groups.map { it.kind })
        assertEquals(listOf("zeta"), groups.single().repositories.map { it.name })
    }

    @Test
    fun `an owner called Other with GitHub repositories stays an owner`() {
        val owned = list.copy(groups = listOf(RepoGroupVm("Other", listOf(2u))))
        assertEquals(listOf(RepositoryGroup.Kind.Owner), groupRepositories(owned).map { it.kind })
    }
}
