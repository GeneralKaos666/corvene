package com.wasimaster.corvene.changes

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.Avatar
import com.wasimaster.corvene.design.BranchName
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.KeyCommand
import com.wasimaster.corvene.design.Label
import com.wasimaster.corvene.design.LocalKeyCommands
import com.wasimaster.corvene.design.LabelVariant
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.Spinner
import com.wasimaster.corvene.design.SpinnerSize
import com.wasimaster.corvene.design.isShortHeight
import com.wasimaster.corvene.ffi.gen.ChangesVm
import com.wasimaster.corvene.ffi.gen.CommitFormVm

/**
 * The bottom of the Changes tab: GHD's Undo bar after a commit, then the
 * docked "Commit to <branch>" bar. The bar opens the commit sheet (GitHub
 * Mobile's "Commit changes" bottom sheet) holding [CommitForm]; the message
 * survives closing the sheet and clears when [ChangesVm.commitNonce] bumps
 * (the engine committed).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CommitPanel(changes: ChangesVm, actions: ChangesActions) {
    val colors = CorveneTheme.colors
    val form = changes.form
    var open by rememberSaveable { mutableStateOf(false) }
    var summary by rememberSaveable(changes.commitNonce) { mutableStateOf("") }
    var description by rememberSaveable(changes.commitNonce) { mutableStateOf("") }
    val branch = form.branch ?: stringResource(R.string.chg_no_branch)
    val commit = {
        actions.commit(summary.trim(), description.trim())
        open = false
    }
    // Ctrl+Enter (GHD's ⌘Enter): opens the form, or commits once it can
    val keys = LocalKeyCommands.current
    val latest by rememberUpdatedState(changes)
    val latestCommit by rememberUpdatedState(commit)
    LaunchedEffect(keys) {
        keys.commands.collect { command ->
            if (command != KeyCommand.Commit) return@collect
            if (open && canCommitNow(latest, summary, latest.form)) latestCommit() else if (latest.includedCount > 0u) open = true
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .background(colors.bgDefault)
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal)),
    ) {
        HorizontalDivider(color = colors.borderMuted)
        // a short window (phone landscape) gives the rows the Undo bar's height unless there are none
        if (form.lastCommitSha != null && (!isShortHeight() || changes.files.isEmpty())) UndoBar(form, actions)
        if (changes.files.isNotEmpty()) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    stringResource(R.string.chg_included, changes.includedCount.toInt(), changes.files.size),
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textSecondary,
                    maxLines = 2,
                )
                PrimerButton(
                    stringResource(R.string.chg_commit_to, branch),
                    { open = true },
                    variant = PrimerButtonVariant.Primary,
                    enabled = changes.includedCount > 0u && !form.committing,
                    leadingIcon = Octicons.GitCommit,
                    modifier = Modifier.testTag(TAG_COMMIT_BAR),
                )
            }
        }
    }
    if (open) {
        ModalBottomSheet(
            onDismissRequest = { open = false },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = colors.bgOverlay,
        ) {
            CommitForm(
                form = form,
                includedCount = changes.includedCount.toInt(),
                summary = summary,
                description = description,
                onSummary = { summary = it },
                onDescription = { description = it },
                onCommit = commit,
                modifier = Modifier.imePadding(),
            )
        }
    }
}

/**
 * The commit form: summary, description, the co-authors (read-only until
 * M-A2), the branch it lands on, and Commit, enabled only with a summary and
 * at least one included file (GHD's rule) while no commit runs.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CommitForm(
    form: CommitFormVm,
    includedCount: Int,
    summary: String,
    description: String,
    onSummary: (String) -> Unit,
    onDescription: (String) -> Unit,
    onCommit: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = CorveneTheme.colors
    val branch = form.branch ?: stringResource(R.string.chg_no_branch)
    val canCommit = includedCount > 0 && summary.isNotBlank() && !form.committing
    Column(
        modifier
            .fillMaxWidth()
            // the sheet is a window of its own: Ctrl+Enter is handled here, not at the root
            .onPreviewKeyEvent { event ->
                val enter = event.key == Key.Enter || event.key == Key.NumPadEnter
                if (event.type == KeyEventType.KeyDown && enter && event.isCtrlPressed && canCommit) {
                    onCommit()
                    true
                } else {
                    false
                }
            }
            .verticalScroll(rememberScrollState())
            .padding(horizontal = CorveneTheme.metrics.gutter)
            .padding(bottom = 16.dp)
            .navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Avatar(form.author ?: "?", size = 24.dp)
            Text(
                stringResource(R.string.chg_commit_title),
                Modifier.weight(1f),
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                color = colors.textPrimary,
            )
            if (form.amending) Label(stringResource(R.string.chg_amending), variant = LabelVariant.Attention)
        }
        PrimerTextField(
            summary,
            onSummary,
            label = stringResource(R.string.chg_summary),
            placeholder = stringResource(R.string.chg_summary_placeholder),
            modifier = Modifier.testTag(TAG_SUMMARY),
        )
        PrimerTextField(
            description,
            onDescription,
            label = stringResource(R.string.chg_description),
            placeholder = stringResource(R.string.chg_description_placeholder),
            singleLine = false,
            minLines = 3,
            maxLines = 8,
            modifier = Modifier.testTag(TAG_DESCRIPTION),
        )
        if (form.coAuthors.isNotEmpty()) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Octicon(Octicons.People, stringResource(R.string.chg_co_authors), tint = OcticonTint.Secondary)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    form.coAuthors.forEach { Label(it, variant = LabelVariant.Accent) }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BranchName(branch, Modifier.weight(1f, fill = false), icon = true)
            Text(
                pluralStringResource(R.plurals.chg_files_to_commit, includedCount, includedCount),
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodySmall,
                color = colors.textSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (form.committing) Spinner(size = SpinnerSize.Small)
            PrimerButton(
                stringResource(R.string.chg_commit),
                onCommit,
                variant = PrimerButtonVariant.Primary,
                enabled = canCommit,
                modifier = Modifier.testTag(TAG_COMMIT),
            )
        }
    }
}

@Composable
private fun UndoBar(form: CommitFormVm, actions: ChangesActions) {
    val colors = CorveneTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = CorveneTheme.metrics.gutter, end = 8.dp, top = 4.dp, bottom = 4.dp)
            .testTag(TAG_UNDO_BAR),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Octicon(Octicons.GitCommit, null, tint = OcticonTint.Secondary)
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.chg_committed), style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
            Text(
                form.lastCommitSummary.orEmpty(),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.textPrimary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        PrimerButton(
            stringResource(R.string.chg_undo),
            actions::undo,
            enabled = !form.committing,
            leadingIcon = Octicons.Undo,
            modifier = Modifier.testTag(TAG_UNDO),
        )
    }
    HorizontalDivider(color = colors.borderMuted)
}

const val TAG_COMMIT_BAR = "chg_commit_bar"
const val TAG_COMMIT = "chg_commit"
const val TAG_SUMMARY = "chg_summary"
const val TAG_DESCRIPTION = "chg_description"
const val TAG_UNDO = "chg_undo"
const val TAG_UNDO_BAR = "chg_undo_bar"

private fun canCommitNow(changes: ChangesVm, summary: String, form: CommitFormVm): Boolean =
    changes.includedCount > 0u && summary.isNotBlank() && !form.committing
