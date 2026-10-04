package com.wasimaster.corvene.mco

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.ffi.gen.McoKindVm
import com.wasimaster.corvene.ffi.gen.McoVm

/**
 * The choose-branch step (GHD's `ChooseBranchDialog`, also Merge into
 * current's branch list): [branches] without [current], filterable; a tap
 * runs [onChoose]. [onNewBranch] adds "New branch" (cherry-pick).
 */
@Composable
fun ChooseBranchSheet(
    title: String,
    branches: List<String>,
    current: String?,
    onChoose: (String) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    onNewBranch: (() -> Unit)? = null,
) {
    var filter by rememberSaveable { mutableStateOf("") }
    val shown = branches.filter { it != current && !it.endsWith("/HEAD") && it.contains(filter.trim(), ignoreCase = true) }
    SelectPanel(
        title = title,
        onDismissRequest = onDismissRequest,
        modifier = modifier.testTag(TAG_CHOOSE),
        filter = filter,
        onFilterChange = { filter = it },
        filterPlaceholder = stringResource(R.string.mco_filter),
        closeDescription = stringResource(R.string.mco_close),
        titleActions = onNewBranch?.let { action ->
            { PrimerButton(stringResource(R.string.mco_new_branch), action, leadingIcon = Octicons.Plus) }
        },
    ) {
        item(key = "h") { ActionListGroupHeader(stringResource(R.string.mco_branches)) }
        items(shown, key = { it }) { name ->
            ActionListItem(
                name,
                onClick = { onChoose(name) },
                modifier = Modifier.testTag("$TAG_CHOOSE_BRANCH$name"),
                leading = { Octicon(Octicons.GitBranch, null, tint = OcticonTint.Secondary) },
            )
        }
        if (shown.isEmpty()) item(key = "empty") { ActionListItem(stringResource(R.string.mco_no_branches), enabled = false) }
        item(key = "pad") { Spacer(Modifier.height(CorveneTheme.spacing.s)) }
    }
}

/** The operation running (GHD's `ProgressDialog`): not dismissable, a bar and the commit being applied. */
@Composable
fun McoProgressDialog(mco: McoVm, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(
            R.string.mco_progress_title,
            stringResource(verbLabel(mco.kind)).replaceFirstChar { it.uppercase() },
            mco.targetBranch.orEmpty(),
        ),
        onDismissRequest = {},
        modifier = modifier.testTag(TAG_PROGRESS),
        dismissible = false,
    ) {
        ProgressBar(if (mco.progressTotal > 0u) mco.progress else null)
        if (mco.progressTotal > 0u) {
            Text(
                stringResource(R.string.mco_progress_step, mco.progressPosition.toInt(), mco.progressTotal.toInt(), mco.progressSummary),
                style = MaterialTheme.typography.bodySmall,
                color = CorveneTheme.colors.textSecondary,
            )
        }
    }
}

/** GHD's `ConfirmAbortDialog`: resolutions would be lost. */
@Composable
fun ConfirmAbortDialog(kind: McoKindVm, onAbort: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    val noun = stringResource(nounLabel(kind))
    PrimerDialog(
        title = stringResource(R.string.mco_confirm_abort_title, noun),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.mco_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.mco_abort, noun),
                onAbort,
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_CONFIRM_ABORT),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.mco_cancel), onDismissRequest) },
    ) {
        Text(
            stringResource(R.string.mco_confirm_abort_body, noun),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textPrimary,
        )
    }
}

/**
 * GHD's `WarnForcePushDialog`: the rebased branch was pushed, so it will
 * need a force push. [onBegin] null: the engine has no way yet to go on
 * past the warning (FFI-REQUESTS), the dialog only informs.
 */
@Composable
fun WarnForcePushDialog(kind: McoKindVm, onBegin: (() -> Unit)?, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    val noun = stringResource(nounLabel(kind))
    PrimerDialog(
        title = stringResource(R.string.mco_warn_force_title, noun),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.mco_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.mco_warn_force_begin, noun),
                { onBegin?.invoke() },
                variant = PrimerButtonVariant.Primary,
                enabled = onBegin != null,
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.mco_cancel), onDismissRequest) },
    ) {
        Text(
            stringResource(R.string.mco_warn_force_body),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textPrimary,
        )
    }
}

/** A new branch's name (cherry-pick › New branch, the engine's CreateBranch step). */
@Composable
fun NewBranchNameDialog(initialName: String, onCreate: (String) -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    var name by rememberSaveable { mutableStateOf(initialName) }
    val clean = name.trim().replace(Regex("\\s+"), "-")
    PrimerDialog(
        title = stringResource(R.string.mco_new_branch_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.mco_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.mco_new_branch_confirm),
                { onCreate(clean) },
                variant = PrimerButtonVariant.Primary,
                enabled = clean.isNotEmpty(),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.mco_cancel), onDismissRequest) },
    ) {
        PrimerTextField(name, { name = it }, label = stringResource(R.string.mco_name), monospace = true)
    }
}

const val TAG_CHOOSE = "mco_choose"
const val TAG_CHOOSE_BRANCH = "mco_choose_branch_"
const val TAG_PROGRESS = "mco_progress"
const val TAG_CONFIRM_ABORT = "mco_confirm_abort"
