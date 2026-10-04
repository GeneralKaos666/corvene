package com.wasimaster.corvene.branches

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
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.SwitchRow

// GHD's branch dialogs (app/src/ui/create-branch, rename-branch,
// delete-branch, stash-changes). The engine opens them as popups
// (`PopupHost` maps the kinds here); the branch panel opens the same
// composables itself, since the FFI has no "request" entry points for them.

/**
 * GHD `CreateBranch`: a name (sanitized as git will take it) and what the
 * branch starts from: [targetSha] when created from a commit in History, else
 * the default branch or the current one when they differ, else the current
 * branch. A full-screen form on compact widths.
 */
@Composable
fun CreateBranchDialog(
    initialName: String,
    current: String?,
    defaultBranch: String?,
    targetSha: String?,
    existing: Set<String>,
    onCreate: (name: String, startPoint: String?) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by rememberSaveable { mutableStateOf(initialName) }
    val choice = defaultBranch != null && current != null && defaultBranch != current && targetSha == null
    var fromDefault by rememberSaveable { mutableStateOf(false) }
    val sanitized = sanitizeBranchName(name)
    val taken = sanitized in existing
    val start = when {
        targetSha != null -> targetSha
        choice && fromDefault -> defaultBranch
        else -> null
    }
    PrimerDialog(
        title = stringResource(R.string.br_create_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_create_confirm),
                { onCreate(sanitized, start) },
                variant = PrimerButtonVariant.Primary,
                enabled = sanitized.isNotEmpty() && !taken,
                modifier = Modifier.testTag(TAG_CREATE_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        PrimerTextField(
            name,
            { name = it },
            label = stringResource(R.string.br_name),
            monospace = true,
            caption = when {
                taken -> stringResource(R.string.br_name_taken, sanitized)
                sanitized != name.trim() && sanitized.isNotEmpty() -> stringResource(R.string.br_name_sanitized, sanitized)
                else -> null
            },
            modifier = Modifier.testTag(TAG_NAME),
        )
        when {
            targetSha != null -> Body(stringResource(R.string.br_create_from_commit, targetSha.take(SHORT_SHA)))
            choice -> {
                Body(stringResource(R.string.br_create_based_on))
                RadioRow(
                    defaultBranch,
                    selected = fromDefault,
                    onClick = { fromDefault = true },
                    caption = stringResource(R.string.br_create_default_caption),
                )
                RadioRow(
                    current,
                    selected = !fromDefault,
                    onClick = { fromDefault = false },
                    caption = stringResource(R.string.br_create_current_caption),
                )
            }
            current != null -> Body(stringResource(R.string.br_create_from_current, current))
            else -> Body(stringResource(R.string.br_create_detached))
        }
    }
}

/** GHD `RenameBranch`. */
@Composable
fun RenameBranchDialog(
    name: String,
    existing: Set<String>,
    onRename: (String) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var value by rememberSaveable { mutableStateOf(name) }
    val sanitized = sanitizeBranchName(value)
    val taken = sanitized != name && sanitized in existing
    PrimerDialog(
        title = stringResource(R.string.br_rename_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_rename_confirm, name),
                { onRename(sanitized) },
                variant = PrimerButtonVariant.Primary,
                enabled = sanitized.isNotEmpty() && sanitized != name && !taken,
                modifier = Modifier.testTag(TAG_RENAME_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        PrimerTextField(
            value,
            { value = it },
            label = stringResource(R.string.br_name),
            monospace = true,
            caption = when {
                taken -> stringResource(R.string.br_name_taken, sanitized)
                sanitized != value.trim() && sanitized.isNotEmpty() -> stringResource(R.string.br_name_sanitized, sanitized)
                else -> null
            },
            modifier = Modifier.testTag(TAG_NAME),
        )
    }
}

/** GHD `DeleteBranch` (+ `DeleteRemoteBranch` folded in: the remote switch). */
@Composable
fun DeleteBranchDialog(
    name: String,
    hasRemote: Boolean,
    onDelete: (includeRemote: Boolean) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var remote by rememberSaveable { mutableStateOf(false) }
    PrimerDialog(
        title = stringResource(R.string.br_delete_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_delete_confirm),
                { onDelete(remote) },
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_DELETE_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_delete_body, name))
        Body(stringResource(R.string.br_delete_undo))
        if (hasRemote) SwitchRow(stringResource(R.string.br_delete_remote), remote, { remote = it })
    }
}

/**
 * GHD `StashAndSwitchBranch`: there are uncommitted changes; leave them
 * stashed on [current] ("stash") or bring them to [target] ("move").
 * [hasStash]: a stash on [current] would be overwritten.
 */
@Composable
fun StashAndSwitchBranchDialog(
    current: String?,
    target: String,
    hasStash: Boolean,
    onSwitch: (strategy: String) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var stash by rememberSaveable { mutableStateOf(true) }
    val here = current ?: stringResource(R.string.br_this_branch)
    PrimerDialog(
        title = stringResource(R.string.br_switch_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_switch_confirm),
                { onSwitch(if (stash) STRATEGY_STASH else STRATEGY_MOVE) },
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_SWITCH_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_switch_body))
        RadioRow(
            stringResource(R.string.br_switch_leave, here),
            selected = stash,
            onClick = { stash = true },
            caption = stringResource(R.string.br_switch_leave_caption),
            modifier = Modifier.testTag(TAG_SWITCH_STASH),
        )
        RadioRow(
            stringResource(R.string.br_switch_bring, target),
            selected = !stash,
            onClick = { stash = false },
            caption = stringResource(R.string.br_switch_bring_caption),
            modifier = Modifier.testTag(TAG_SWITCH_MOVE),
        )
        if (stash && hasStash) {
            Text(
                stringResource(R.string.br_switch_overwrite_warning),
                style = MaterialTheme.typography.bodySmall,
                color = CorveneTheme.colors.attention.fg,
            )
        }
    }
}

/** GHD `ConfirmOverwriteStash`: switching stashes the changes over the branch's existing stash. */
@Composable
fun ConfirmOverwriteStashDialog(onOverwrite: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.br_overwrite_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(stringResource(R.string.br_overwrite_confirm), onOverwrite, variant = PrimerButtonVariant.Danger)
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_overwrite_body))
    }
}

/** Corvene `864-confirm-branch-switch`: "Switch to <branch>?" */
@Composable
fun ConfirmSwitchBranchDialog(target: String, onSwitch: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.br_confirm_switch_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(stringResource(R.string.br_switch_confirm), onSwitch, variant = PrimerButtonVariant.Primary)
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_confirm_switch_body, target))
    }
}

@Composable
internal fun Body(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = CorveneTheme.colors.textPrimary)
}

/** `checkoutBranch` strategies. */
const val STRATEGY_STASH = "stash"
const val STRATEGY_MOVE = "move"

private const val SHORT_SHA = 7

const val TAG_NAME = "br_name"
const val TAG_CREATE_CONFIRM = "br_create_confirm"
const val TAG_RENAME_CONFIRM = "br_rename_confirm"
const val TAG_DELETE_CONFIRM = "br_delete_confirm"
const val TAG_SWITCH_CONFIRM = "br_switch_confirm"
const val TAG_SWITCH_STASH = "br_switch_stash"
const val TAG_SWITCH_MOVE = "br_switch_move"
