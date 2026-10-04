package com.wasimaster.corvene.settings

import com.wasimaster.corvene.ffi.gen.FlagVm
import com.wasimaster.corvene.ffi.gen.FlagsVm

/** The desktop dialog's All / On / Off switch: every flag, the ones that deviate from GitHub Desktop, the ones that do not. */
enum class FlagFilter { All, On, Off }

/** A category's flags as listed: its title and the rows. */
data class FlagGroup(val category: String, val flags: List<FlagVm>)

private const val BUG_FIX = "Bug fix"

/**
 * The rows to list, as the desktop Flags dialog picks them
 * (`crates/corvene-ui/src/dialogs/flags.rs::visible_rows`): bug fixes only
 * with [showBugFixes], the [filter]'s state, the [query] in the title,
 * summary, GitHub Desktop's behaviour or the `NNN-slug` ident (any case);
 * grouped by category in the engine's category order.
 */
fun FlagsVm.visibleGroups(query: String, filter: FlagFilter, showBugFixes: Boolean): List<FlagGroup> {
    val q = query.trim().lowercase()
    val shown = flags.filter { flag ->
        (showBugFixes || flag.nature != BUG_FIX) &&
            when (filter) {
                FlagFilter.All -> true
                FlagFilter.On -> flag.isOn
                FlagFilter.Off -> !flag.isOn
            } &&
            (q.isEmpty() || flag.matches(q))
    }
    val byCategory = shown.groupBy { it.category }
    val order = categories + byCategory.keys.filterNot { it in categories }
    return order.mapNotNull { category -> byCategory[category]?.let { FlagGroup(category, it) } }
}

private fun FlagVm.matches(q: String): Boolean =
    title.lowercase().contains(q) ||
        summary.lowercase().contains(q) ||
        ghdBehaviour.lowercase().contains(q) ||
        ident.lowercase().contains(q)

/** Whether a flag needs the app restarted before its value takes effect. */
val FlagsVm.restartPending: Boolean get() = flags.any { it.restartPending }

/** Whether any flag is set apart from the preset (Reset all has something to do). */
val FlagsVm.anyOverridden: Boolean get() = flags.any { it.overridden }

/** The current preset's title, for the preset row. */
val FlagsVm.presetTitle: String get() = presets.firstOrNull { it.slug == preset }?.title ?: preset
