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
 * The engine's grouping (`RepoListVm.groups`: GHD's `groupRepositories`,
 * Recent / one per GitHub owner / Other, flag 209 deciding how many recent
 * ones) with the rows resolved, narrowed by [filter] (case-insensitive,
 * name or `owner/name`). A filter drops the Recent group so no repository
 * shows twice (GHD filters the flat list).
 */
fun groupRepositories(list: RepoListVm, filter: String = ""): List<RepositoryGroup> {
    val byId = list.repositories.associateBy { it.id }
    val needle = filter.trim()
    return list.groups.mapIndexedNotNull { index, group ->
        val repos = group.ids.mapNotNull(byId::get)
            .filter { needle.isEmpty() || it.name.contains(needle, ignoreCase = true) || it.github?.contains(needle, true) == true }
        val kind = when {
            index == 0 && group.title == RECENT && list.groups.size > 1 -> RepositoryGroup.Kind.Recent
            repos.all { it.github == null } && group.title == OTHER -> RepositoryGroup.Kind.Other
            else -> RepositoryGroup.Kind.Owner
        }
        val hidden = repos.isEmpty() || kind == RepositoryGroup.Kind.Recent && needle.isNotEmpty()
        if (hidden) null else RepositoryGroup(kind, group.title, repos)
    }
}

/** The engine's titles for the two fixed groups (crates/corvene-ffi/src/vm/repo_list.rs). */
private const val RECENT = "Recent"
private const val OTHER = "Other"
