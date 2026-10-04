package com.wasimaster.corvene

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.PopupVm
import com.wasimaster.corvene.ffi.rememberCoreQuery

/**
 * The engine's open dialog (`popup()`, GHD's popup stack), one `when` over
 * GHD's `PopupType` names. M-A1 draws [Error]; every other kind gets a
 * temporary [GenericPopup] showing its fields, until its own dialog lands.
 * Closing sends `closePopup()`.
 */
@Composable
fun PopupHost() {
    val core = LocalCore.current
    val popup by rememberCoreQuery { popup() ?: NoPopup }
    val value = popup.value?.takeIf { it.kind.isNotEmpty() } ?: return
    val close = { core.dispatch { closePopup() } }
    when (value.kind) {
        "Error" -> Error(value, close)
        else -> GenericPopup(value, close)
    }
}

/** GHD's error dialog: the title, the message, and git's output behind "Show details". */
@Composable
private fun Error(popup: PopupVm, onClose: () -> Unit) {
    var details by rememberSaveable(popup) { mutableStateOf(false) }
    val output = popup.field("details") ?: popup.field("output")
    PrimerDialog(
        title = popup.field("title") ?: stringResource(R.string.app_error),
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = { PrimerButton(stringResource(R.string.app_close), onClose, variant = PrimerButtonVariant.Primary) },
    ) {
        Text(popup.field("text") ?: popup.field("message").orEmpty(), style = MaterialTheme.typography.bodyMedium, color = CorveneTheme.colors.textPrimary)
        if (!output.isNullOrBlank()) {
            PrimerButton(
                stringResource(if (details) R.string.app_hide_details else R.string.app_show_details),
                { details = !details },
                variant = PrimerButtonVariant.Link,
            )
            if (details) {
                popup.field("command")?.let {
                    Text("$ $it", style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textSecondary)
                }
                Text(
                    output,
                    Modifier.horizontalScroll(rememberScrollState()),
                    style = CorveneTheme.textStyles.codeSmall,
                    color = CorveneTheme.colors.textPrimary,
                    softWrap = false,
                )
            }
        }
    }
}

/** Any other dialog, until it has its own: the kind, the payload, Close. */
@Composable
private fun GenericPopup(popup: PopupVm, onClose: () -> Unit) {
    PrimerDialog(
        title = popup.kind,
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = { PrimerButton(stringResource(R.string.app_close), onClose) },
    ) {
        Text(
            stringResource(R.string.app_popup_later),
            style = MaterialTheme.typography.bodySmall,
            color = CorveneTheme.colors.textSecondary,
        )
        popup.fields.forEach { field ->
            Column {
                Text(field.key, style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold))
                Text(field.value, style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textPrimary)
            }
        }
        popup.lists.forEach { list ->
            Column {
                Text(list.key, style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold))
                list.items.forEach { Text(it, style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textPrimary) }
            }
        }
    }
}

private fun PopupVm.field(key: String): String? = fields.firstOrNull { it.key == key }?.value

/** `rememberCoreQuery` wants a value; this stands for "no dialog". */
private val NoPopup = PopupVm(kind = "", repo = null, fields = emptyList(), lists = emptyList())
