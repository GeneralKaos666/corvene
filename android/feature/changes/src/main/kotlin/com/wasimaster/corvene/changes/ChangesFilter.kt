package com.wasimaster.corvene.changes

import com.wasimaster.corvene.ffi.gen.ChangedFileVm
import com.wasimaster.corvene.ffi.gen.ChangesVm
import com.wasimaster.corvene.ffi.gen.FileStatusVm
import com.wasimaster.corvene.ffi.gen.IncludeVm

/**
 * GHD's file list filter options (`FilterOption`; Renamed is Corvene's flag
 * 705), with the key `toggleFilterOption` takes.
 */
enum class FilterOption(val key: String) {
    Included("included"),
    Excluded("excluded"),
    New("new"),
    Modified("modified"),
    Deleted("deleted"),
    Renamed("renamed"),
}

/** The options the engine has switched on. */
fun ChangesVm.activeFilters(): Set<FilterOption> = buildSet {
    if (filterIncluded) add(FilterOption.Included)
    if (filterExcluded) add(FilterOption.Excluded)
    if (filterNew) add(FilterOption.New)
    if (filterModified) add(FilterOption.Modified)
    if (filterDeleted) add(FilterOption.Deleted)
    if (filterRenamed) add(FilterOption.Renamed)
}

/**
 * The files the list shows: every active option must match
 * (crates/corvene-core/src/filter.rs `matches_options`). The engine's view
 * model carries the whole list; the filter is applied here.
 */
fun ChangesVm.visibleFiles(): List<ChangedFileVm> {
    val active = activeFilters()
    if (active.isEmpty()) return files
    return files.filter { file -> active.all { it.matches(file) } }
}

private fun FilterOption.matches(file: ChangedFileVm): Boolean = when (this) {
    FilterOption.Included -> file.include != IncludeVm.NONE
    FilterOption.Excluded -> file.include == IncludeVm.NONE
    FilterOption.New -> file.status == FileStatusVm.NEW || file.status == FileStatusVm.UNTRACKED
    FilterOption.Modified -> file.status == FileStatusVm.MODIFIED
    FilterOption.Deleted -> file.status == FileStatusVm.DELETED
    FilterOption.Renamed -> file.status == FileStatusVm.RENAMED
}
