package com.wasimaster.corvene.repositories

import androidx.compose.runtime.Immutable
import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm

/** A heading and its rows; [title] is the owner for [Kind.Owner] (the others use string resources). */
@Immutable
data class RepositoryGroup(val kind: Kind, val title: String, val repositories: List<RepoVm>) {
    enum class Kind { Recent, Owner, Other }
}

/**
 * GHD's `groupRepositories` (crates/corvene-ui/src/repository_list.rs `groups`
 * without a filter): Recent (the three most recently opened, when there is more
 * than one repository), then one group per GitHub owner, then Other; names
 * sorted case-insensitively within a group.
 *
 * Kotlin-side until the engine's view model carries the groups
 * (android/FFI-REQUESTS.md).
 */
fun groupRepositories(list: RepoListVm, recentCount: Int = RECENT_COUNT): List<RepositoryGroup> {
    val byId = list.repositories.associateBy { it.id }
    val groups = mutableListOf<RepositoryGroup>()
    val recent = list.recent.take(recentCount).mapNotNull(byId::get)
    if (recent.isNotEmpty() && list.repositories.size > 1) {
        groups += RepositoryGroup(RepositoryGroup.Kind.Recent, "", recent)
    }
    val sorted = list.repositories.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
    sorted.filter { it.github != null }
        .groupBy { it.github.orEmpty().substringBefore('/') }
        .toSortedMap(String.CASE_INSENSITIVE_ORDER)
        .forEach { (owner, repos) -> groups += RepositoryGroup(RepositoryGroup.Kind.Owner, owner, repos) }
    val other = sorted.filter { it.github == null }
    if (other.isNotEmpty()) groups += RepositoryGroup(RepositoryGroup.Kind.Other, "", other)
    return groups
}

private const val RECENT_COUNT = 3
