package com.wasimaster.corvene.changes

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.ChipRow
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.FilePath
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonColored
import com.wasimaster.corvene.design.OcticonIcon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerCheckbox
import com.wasimaster.corvene.design.PrimerChip
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.StateLabel
import com.wasimaster.corvene.design.StateLabelState
import com.wasimaster.corvene.ffi.gen.ChangedFileVm
import com.wasimaster.corvene.ffi.gen.ChangesVm
import com.wasimaster.corvene.ffi.gen.FileStatusVm
import com.wasimaster.corvene.ffi.gen.IncludeVm
import kotlinx.coroutines.launch

/** What the Changes list sends back. */
interface ChangesActions {
    fun select(path: String)

    fun toggleIncluded(path: String)

    fun discard(paths: List<String>)

    fun toggleFilter(option: FilterOption)

    fun clearFilters()

    fun commit(summary: String, description: String)

    fun undo()

    fun restoreStash()
}

/**
 * The Changes tab (GHD's ChangesSidebar): the conflicts and stash banners,
 * the filter chips, the changed files (include box with the partial state,
 * status icon, path, +/− counts; tap opens the diff, swipe right toggles the
 * include box, swipe left discards after [confirmDiscard]'s prompt), the Undo
 * bar after a commit and the docked commit panel. No changes: a Blankslate
 * with suggested next steps.
 */
@Composable
fun ChangesScreen(
    changes: ChangesVm,
    confirmDiscard: Boolean,
    actions: ChangesActions,
    modifier: Modifier = Modifier,
) {
    var discarding by rememberSaveable { mutableStateOf<String?>(null) }
    val files = remember(changes) { changes.visibleFiles() }
    val active = remember(changes) { changes.activeFilters() }
    val requestDiscard: (String) -> Unit = { path -> if (confirmDiscard) discarding = path else actions.discard(listOf(path)) }
    Column(modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas)) {
        if (changes.conflicts > 0u) {
            Flash(
                pluralStringResource(R.plurals.chg_conflicts, changes.conflicts.toInt(), changes.conflicts.toInt()),
                variant = FlashVariant.Danger,
                flush = true,
            )
        }
        if (changes.stashCount > 0u) {
            Flash(
                stringResource(R.string.chg_stash),
                icon = Octicons.Stack,
                flush = true,
                modifier = Modifier.testTag(TAG_STASH),
                action = {
                    PrimerButton(stringResource(R.string.chg_stash_restore), actions::restoreStash, variant = PrimerButtonVariant.Link)
                },
            )
        }
        if (changes.files.isEmpty()) {
            NoChanges(Modifier.weight(1f))
        } else {
            FilterChips(active, actions)
            ListHeader(changes, files.size)
            LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag(TAG_FILES)) {
                items(files, key = { it.path }, contentType = { "file" }) { file ->
                    FileRow(file, actions, onDiscard = { requestDiscard(file.path) })
                }
                if (files.isEmpty()) {
                    item(key = "filtered") {
                        Text(
                            stringResource(R.string.chg_filtered_empty),
                            Modifier.fillMaxWidth().padding(24.dp),
                            style = MaterialTheme.typography.bodyMedium,
                            color = CorveneTheme.colors.textSecondary,
                        )
                    }
                }
            }
        }
        CommitPanel(changes, actions)
    }
    val target = discarding
    if (target != null) {
        PrimerDialog(
            title = stringResource(R.string.chg_discard_title),
            onDismissRequest = { discarding = null },
            closeDescription = stringResource(R.string.chg_close),
            confirmButton = {
                PrimerButton(
                    stringResource(R.string.chg_discard_confirm),
                    {
                        discarding = null
                        actions.discard(listOf(target))
                    },
                    variant = PrimerButtonVariant.Danger,
                    modifier = Modifier.testTag(TAG_DISCARD_CONFIRM),
                )
            },
            dismissButton = { PrimerButton(stringResource(R.string.chg_cancel), { discarding = null }) },
        ) {
            Text(stringResource(R.string.chg_discard_message, target), color = CorveneTheme.colors.textPrimary)
        }
    }
}

@Composable
private fun FilterChips(active: Set<FilterOption>, actions: ChangesActions) {
    ChipRow {
        FilterOption.entries.forEach { option ->
            PrimerChip(
                stringResource(option.label()),
                selected = option in active,
                onClick = { actions.toggleFilter(option) },
                modifier = Modifier.testTag("$TAG_FILTER${option.key}"),
            )
        }
        if (active.isNotEmpty()) {
            PrimerButton(stringResource(R.string.chg_filter_clear), actions::clearFilters, variant = PrimerButtonVariant.Link)
        }
    }
}

private fun FilterOption.label(): Int = when (this) {
    FilterOption.Included -> R.string.chg_filter_included
    FilterOption.Excluded -> R.string.chg_filter_excluded
    FilterOption.New -> R.string.chg_filter_new
    FilterOption.Modified -> R.string.chg_filter_modified
    FilterOption.Deleted -> R.string.chg_filter_deleted
    FilterOption.Renamed -> R.string.chg_filter_renamed
}

@Composable
private fun ListHeader(changes: ChangesVm, shown: Int) {
    val colors = CorveneTheme.colors
    Row(
        Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val total = changes.files.size
        Text(
            if (shown == total) {
                pluralStringResource(R.plurals.chg_changed_files, total, total)
            } else {
                stringResource(R.string.chg_changed_files_filtered, shown, total)
            },
            Modifier.weight(1f),
            style = MaterialTheme.typography.labelLarge,
            color = colors.textSecondary,
        )
        if (changes.conflicts > 0u) {
            StateLabel(
                pluralStringResource(R.plurals.chg_conflict_badge, changes.conflicts.toInt(), changes.conflicts.toInt()),
                StateLabelState.Closed,
                icon = Octicons.Alert,
                modifier = Modifier.testTag(TAG_CONFLICTS),
            )
        }
    }
    HorizontalDivider(color = colors.borderMuted)
}

@Composable
private fun FileRow(file: ChangedFileVm, actions: ChangesActions, onDiscard: () -> Unit) {
    val scope = rememberCoroutineScope()
    val state = rememberSwipeToDismissBoxState()
    val colors = CorveneTheme.colors
    val includeLabel = stringResource(if (file.include == IncludeVm.NONE) R.string.chg_include else R.string.chg_exclude)
    val discardLabel = stringResource(R.string.chg_discard)
    SwipeToDismissBox(
        state = state,
        backgroundContent = {
            val (bg, icon, alignment) = when (state.dismissDirection) {
                SwipeToDismissBoxValue.StartToEnd -> Triple(colors.accent.emphasis, Octicons.Check, Alignment.CenterStart)
                SwipeToDismissBoxValue.EndToStart -> Triple(colors.danger.emphasis, Octicons.Trash, Alignment.CenterEnd)
                SwipeToDismissBoxValue.Settled -> Triple(colors.bgCanvas, null, Alignment.Center)
            }
            Box(Modifier.fillMaxSize().background(bg).padding(horizontal = 24.dp), contentAlignment = alignment) {
                if (icon != null) OcticonColored(icon, null, colors.textOnEmphasis, size = 24.dp)
            }
        },
        onDismiss = { direction ->
            when (direction) {
                SwipeToDismissBoxValue.StartToEnd -> actions.toggleIncluded(file.path)
                SwipeToDismissBoxValue.EndToStart -> onDiscard()
                SwipeToDismissBoxValue.Settled -> Unit
            }
            scope.launch { state.reset() }
        },
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .background(if (file.selected) colors.bgSelected else colors.bgCanvas)
                .semantics {
                    selected = file.selected
                    customActions = listOf(
                        CustomAccessibilityAction(includeLabel) {
                            actions.toggleIncluded(file.path)
                            true
                        },
                        CustomAccessibilityAction(discardLabel) {
                            onDiscard()
                            true
                        },
                    )
                }
                .clickable(role = Role.Button) { actions.select(file.path) }
                .heightIn(min = CorveneTheme.metrics.rowHeight.coerceAtMost(52.dp))
                .padding(end = CorveneTheme.metrics.gutter)
                .testTag("$TAG_FILE${file.path}"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            PrimerCheckbox(
                file.include.toToggleable(),
                onClick = { actions.toggleIncluded(file.path) },
                contentDescription = stringResource(R.string.chg_include_file, file.path),
                modifier = Modifier.padding(start = 4.dp).testTag("$TAG_INCLUDE${file.path}"),
            )
            StatusIcon(file.status)
            FilePath(file.path, Modifier.weight(1f), oldPath = file.oldPath, style = MaterialTheme.typography.bodyMedium)
            val added = file.linesAdded
            val deleted = file.linesDeleted
            if (added != null && deleted != null && (added > 0u || deleted > 0u)) LineStats(added, deleted)
        }
    }
}

/** GHD's file status glyphs in the semantic colours. */
@Composable
internal fun StatusIcon(status: FileStatusVm, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    val (icon, color, label) = when (status) {
        FileStatusVm.NEW, FileStatusVm.UNTRACKED, FileStatusVm.COPIED -> Triple(Octicons.DiffAdded, colors.fileNew, R.string.chg_status_new)
        FileStatusVm.MODIFIED -> Triple(Octicons.DiffModified, colors.fileModified, R.string.chg_status_modified)
        FileStatusVm.DELETED -> Triple(Octicons.DiffRemoved, colors.fileDeleted, R.string.chg_status_deleted)
        FileStatusVm.RENAMED -> Triple(Octicons.DiffRenamed, colors.fileRenamed, R.string.chg_status_renamed)
        FileStatusVm.CONFLICTED -> Triple(Octicons.Alert, colors.fileConflicted, R.string.chg_status_conflicted)
    }
    OcticonColored(icon, stringResource(label), color, modifier)
}

/** GHD's NoChanges view: the blank slate and the suggested next steps (placeholders until M-A3). */
@Composable
private fun NoChanges(modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 16.dp)) {
        Blankslate(
            icon = Octicons.CheckCircle,
            title = stringResource(R.string.chg_empty_title),
            description = stringResource(R.string.chg_empty_description),
            modifier = Modifier.testTag(TAG_EMPTY),
        )
        Column(Modifier.padding(horizontal = CorveneTheme.metrics.gutter), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            SuggestedAction(Octicons.ArrowDown, R.string.chg_suggest_pull, R.string.chg_suggest_pull_body, R.string.chg_suggest_pull_button)
            SuggestedAction(
                Octicons.CodeSquare,
                R.string.chg_suggest_editor,
                R.string.chg_suggest_editor_body,
                R.string.chg_suggest_editor_button,
            )
            SuggestedAction(
                Octicons.MarkGithub,
                R.string.chg_suggest_github,
                R.string.chg_suggest_github_body,
                R.string.chg_suggest_github_button,
            )
        }
    }
}

@Composable
private fun SuggestedAction(icon: OcticonIcon, title: Int, body: Int, button: Int) {
    val colors = CorveneTheme.colors
    val shape = RoundedCornerShape(CorveneTheme.metrics.cornerLarge)
    Row(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.bgDefault)
            .border(1.dp, colors.borderDefault, shape)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Octicon(icon, null, tint = OcticonTint.Secondary, size = 24.dp)
        Column(Modifier.weight(1f)) {
            Text(
                stringResource(title),
                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                color = colors.textPrimary,
            )
            Text(stringResource(body), style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
        }
        // M-A3 wires pull, the editor and the browser
        PrimerButton(stringResource(button), {}, enabled = false)
    }
}

const val TAG_FILES = "chg_files"
const val TAG_FILE = "chg_file_"
const val TAG_INCLUDE = "chg_include_"
const val TAG_FILTER = "chg_filter_"
const val TAG_STASH = "chg_stash"
const val TAG_CONFLICTS = "chg_conflicts"
const val TAG_EMPTY = "chg_empty"
const val TAG_DISCARD_CONFIRM = "chg_discard_confirm"
