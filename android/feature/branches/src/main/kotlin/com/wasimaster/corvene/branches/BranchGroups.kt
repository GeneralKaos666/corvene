package com.wasimaster.corvene.branches

import com.wasimaster.corvene.ffi.gen.BranchVm
import com.wasimaster.corvene.ffi.gen.BranchesVm

/**
 * GHD's branch list groups (`groupBranches`): the default branch, the recent
 * ones (most recent first, without the default), the other local branches by
 * name, then the remote branches that have no local branch of the same name
 * (Corvene keeps them in their own group, collapsed until expanded or a filter
 * is typed). [filter] matches anywhere in the name, ignoring case.
 */
data class BranchGroups(
    val default: List<BranchVm>,
    val recent: List<BranchVm>,
    val other: List<BranchVm>,
    val remote: List<BranchVm>,
) {
    val isEmpty: Boolean get() = default.isEmpty() && recent.isEmpty() && other.isEmpty() && remote.isEmpty()
}

fun groupBranches(vm: BranchesVm, filter: String): BranchGroups {
    val query = filter.trim()
    fun matches(branch: BranchVm) = query.isEmpty() || branch.name.contains(query, ignoreCase = true)
    val locals = vm.branches.filter { !it.remote }
    val shown = locals.filter(::matches)
    val default = shown.filter { it.name == vm.defaultBranch }
    val recent = vm.recent.filter { it != vm.defaultBranch }.mapNotNull { name -> shown.firstOrNull { it.name == name } }
    val grouped = (default + recent).map { it.name }.toSet()
    val other = shown.filter { it.name !in grouped }.sortedBy { it.name.lowercase() }
    val localNames = locals.map { it.name }.toSet()
    val remote = vm.branches
        .filter { it.remote && matches(it) && !it.name.endsWith("/HEAD") && it.name.substringAfter('/') !in localNames }
        .sortedBy { it.name.lowercase() }
    return BranchGroups(default, recent, other, remote)
}

/**
 * A branch name as git will take it (GHD `sanitizedRefName`): runs of
 * whitespace and characters git refuses become `-`, and no leading `-` or
 * trailing `.lock`/`/`/`.`.
 */
fun sanitizeBranchName(input: String): String {
    var name = input.trim().replace(INVALID, "-").replace(Regex("\\.{2,}"), "-").replace(Regex("-{2,}"), "-")
    name = name.trimStart('-', '.', '/')
    while (name.endsWith(".lock") || name.endsWith("/") || name.endsWith(".")) {
        name = name.removeSuffix(".lock").trimEnd('/', '.')
    }
    return name
}

private val INVALID = Regex("""[\s~^:?*\[\\\x00-\x1f\x7f]+|@\{""")
