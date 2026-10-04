package com.wasimaster.corvene.mco

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.FilePath
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.OcticonColored
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.design.PrimerTopAppBar
import com.wasimaster.corvene.design.SegmentedControl
import com.wasimaster.corvene.ffi.gen.ConflictedFileVm
import com.wasimaster.corvene.ffi.gen.ConflictsVm
import com.wasimaster.corvene.ffi.gen.McoKindVm
import com.wasimaster.corvene.ffi.gen.ResolutionVm

/** What the conflicts screen sends back. */
interface ConflictsActions {
    /** Ours / Theirs for [path]; null returns it to a manual resolution. */
    fun resolve(path: String, resolution: ResolutionVm?)

    /** Abort (the route asks first when resolutions would be lost). */
    fun abort()

    /** Continue the merge / rebase / cherry-pick (every file resolved). */
    fun continueOperation()

    /** Close: the conflicts stay, the banner offers them again. */
    fun close()
}

/**
 * GHD's conflicts dialog (MergeConflictsDialog / rebase / cherry-pick) as a
 * screen: what is being resolved, then each conflicted file with Ours |
 * Theirs (the engine's manual resolution) and "Open in editor" (disabled:
 * editors arrive with M-A3), resolved files marked; Abort and Continue at the
 * bottom (Continue waits for every file).
 */
@Composable
fun ConflictsScreen(conflicts: ConflictsVm, actions: ConflictsActions, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    Column(modifier.fillMaxSize().background(colors.bgCanvas).testTag(TAG_CONFLICTS)) {
        PrimerTopAppBar(
            title = stringResource(R.string.mco_conflicts_title),
            subtitle = stringResource(kindLabel(conflicts.kind)),
            onBack = actions::close,
            backDescription = stringResource(R.string.mco_close),
        )
        val target = conflicts.targetBranch ?: conflicts.currentBranch
        Flash(
            if (target != null) {
                stringResource(R.string.mco_conflicts_body_branch, stringResource(verbLabel(conflicts.kind)), target)
            } else {
                stringResource(R.string.mco_conflicts_body, stringResource(verbLabel(conflicts.kind)))
            },
            variant = if (conflicts.unresolvedCount > 0u) FlashVariant.Warning else FlashVariant.Success,
            flush = true,
        )
        Text(
            if (conflicts.unresolvedCount > 0u) {
                pluralStringResource(R.plurals.mco_unresolved, conflicts.unresolvedCount.toInt(), conflicts.unresolvedCount.toInt())
            } else {
                stringResource(R.string.mco_all_resolved)
            },
            Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp).testTag(TAG_UNRESOLVED),
            style = MaterialTheme.typography.labelLarge,
            color = colors.textSecondary,
        )
        HorizontalDivider(color = colors.borderMuted)
        LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
            items(conflicts.files, key = { it.path }) { file ->
                ConflictRow(file, actions)
                HorizontalDivider(color = colors.borderMuted)
            }
        }
        HorizontalDivider(color = colors.borderMuted)
        Row(
            Modifier
                .fillMaxWidth()
                .background(colors.bgDefault)
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal))
                .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            PrimerButton(
                stringResource(R.string.mco_abort, stringResource(nounLabel(conflicts.kind))),
                actions::abort,
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_ABORT),
            )
            PrimerButton(
                stringResource(R.string.mco_continue, stringResource(nounLabel(conflicts.kind))),
                actions::continueOperation,
                variant = PrimerButtonVariant.Primary,
                enabled = conflicts.unresolvedCount == 0u,
                modifier = Modifier.testTag(TAG_CONTINUE),
            )
        }
    }
}

@Composable
private fun ConflictRow(file: ConflictedFileVm, actions: ConflictsActions) {
    val colors = CorveneTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 10.dp).testTag("$TAG_FILE${file.path}"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (file.unresolved) {
                OcticonColored(Octicons.Alert, stringResource(R.string.mco_file_conflicted), colors.fileConflicted)
            } else {
                OcticonColored(Octicons.CheckCircle, stringResource(R.string.mco_file_resolved), colors.success.fg)
            }
            FilePath(file.path, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
        }
        Text(
            when (file.resolution) {
                ResolutionVm.OURS -> stringResource(R.string.mco_using_ours)
                ResolutionVm.THEIRS -> stringResource(R.string.mco_using_theirs)
                null -> stringResource(if (file.unresolved) R.string.mco_has_conflicts else R.string.mco_no_conflicts)
            },
            style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
            color = if (file.unresolved) colors.danger.fg else colors.success.fg,
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SegmentedControl(
                listOf(stringResource(R.string.mco_ours), stringResource(R.string.mco_theirs)),
                selectedIndex = when (file.resolution) {
                    ResolutionVm.OURS -> 0
                    ResolutionVm.THEIRS -> 1
                    null -> -1
                },
                onSelect = { index ->
                    val chosen = if (index == 0) ResolutionVm.OURS else ResolutionVm.THEIRS
                    actions.resolve(file.path, if (chosen == file.resolution) null else chosen)
                },
                modifier = Modifier.weight(1f).testTag("$TAG_RESOLVE${file.path}"),
                fill = true,
            )
            // editors arrive with M-A3
            PrimerIconButton(Octicons.CodeSquare, stringResource(R.string.mco_open_editor), {}, enabled = false)
        }
    }
}

internal fun kindLabel(kind: McoKindVm): Int = when (kind) {
    McoKindVm.MERGE -> R.string.mco_kind_merge
    McoKindVm.REBASE -> R.string.mco_kind_rebase
    McoKindVm.CHERRY_PICK -> R.string.mco_kind_cherry_pick
    McoKindVm.SQUASH -> R.string.mco_kind_squash
    McoKindVm.REORDER -> R.string.mco_kind_reorder
}

internal fun verbLabel(kind: McoKindVm): Int = when (kind) {
    McoKindVm.MERGE -> R.string.mco_verb_merge
    McoKindVm.REBASE -> R.string.mco_verb_rebase
    McoKindVm.CHERRY_PICK -> R.string.mco_verb_cherry_pick
    McoKindVm.SQUASH -> R.string.mco_verb_squash
    McoKindVm.REORDER -> R.string.mco_verb_reorder
}

internal fun nounLabel(kind: McoKindVm): Int = when (kind) {
    McoKindVm.MERGE -> R.string.mco_noun_merge
    McoKindVm.REBASE -> R.string.mco_noun_rebase
    McoKindVm.CHERRY_PICK -> R.string.mco_noun_cherry_pick
    McoKindVm.SQUASH -> R.string.mco_noun_squash
    McoKindVm.REORDER -> R.string.mco_noun_reorder
}

const val TAG_CONFLICTS = "mco_conflicts"
const val TAG_UNRESOLVED = "mco_unresolved"
const val TAG_FILE = "mco_file_"
const val TAG_RESOLVE = "mco_resolve_"
const val TAG_ABORT = "mco_abort"
const val TAG_CONTINUE = "mco_continue"
