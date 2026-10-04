package com.wasimaster.corvene.history

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

// GHD's history dialogs (app/src/ui/create-tag, checkout-commit, reset).
// The engine opens them as popups (`PopupHost` maps the kinds here); the
// commit menu opens them itself, since the FFI exposes the confirmed
// actions only (`checkoutCommit`, `resetToCommit`).

/** GHD `CreateTag`: a tag name (and an optional message, an annotated tag) on [sha]. */
@Composable
fun CreateTagDialog(
    sha: String,
    onCreate: (name: String, message: String) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by rememberSaveable { mutableStateOf("") }
    var message by rememberSaveable { mutableStateOf("") }
    val tag = name.trim().replace(Regex("\\s+"), "-")
    PrimerDialog(
        title = stringResource(R.string.hist_tag_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.hist_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.hist_tag_confirm),
                { onCreate(tag, message.trim()) },
                variant = PrimerButtonVariant.Primary,
                enabled = tag.isNotEmpty() && tag.length <= MAX_TAG,
                modifier = Modifier.testTag(TAG_TAG_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.hist_cancel), onDismissRequest) },
    ) {
        Text(
            stringResource(R.string.hist_tag_body, sha.take(SHORT_SHA)),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textPrimary,
        )
        PrimerTextField(
            name,
            { name = it },
            label = stringResource(R.string.hist_tag_name),
            monospace = true,
            caption = if (tag != name.trim() && tag.isNotEmpty()) stringResource(R.string.hist_tag_sanitized, tag) else null,
            modifier = Modifier.testTag(TAG_TAG_NAME),
        )
        PrimerTextField(
            message,
            { message = it },
            label = stringResource(R.string.hist_tag_message),
            singleLine = false,
            minLines = 2,
        )
    }
}

/** GHD `ConfirmCheckoutCommit`: the detached HEAD warning. */
@Composable
fun ConfirmCheckoutCommitDialog(onCheckout: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.hist_checkout_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.hist_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.hist_checkout_confirm),
                onCheckout,
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_CHECKOUT_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.hist_cancel), onDismissRequest) },
    ) {
        Text(
            stringResource(R.string.hist_checkout_body),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textPrimary,
        )
    }
}

/** GHD `WarningBeforeReset` (Corvene's `ResetToCommit` popup): `reset --mixed` keeps the changes, unstaged. */
@Composable
fun WarningBeforeResetDialog(onReset: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.hist_reset_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.hist_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.hist_reset_confirm),
                onReset,
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_RESET_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.hist_cancel), onDismissRequest) },
    ) {
        Text(stringResource(R.string.hist_reset_body), style = MaterialTheme.typography.bodyMedium, color = CorveneTheme.colors.textPrimary)
    }
}

private const val SHORT_SHA = 7
private const val MAX_TAG = 245

const val TAG_TAG_NAME = "hist_tag_name"
const val TAG_TAG_CONFIRM = "hist_tag_confirm"
const val TAG_CHECKOUT_CONFIRM = "hist_checkout_confirm"
const val TAG_RESET_CONFIRM = "hist_reset_confirm"
