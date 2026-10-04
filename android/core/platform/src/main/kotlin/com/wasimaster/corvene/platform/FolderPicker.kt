package com.wasimaster.corvene.platform

import android.content.Context
import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Opens a system picker; see [rememberFolderPicker] and [rememberFilePicker]. */
fun interface FolderPicker {
    fun pick()
}

/**
 * The system folder picker (Storage Access Framework). [onResult] gets the
 * resolved path ([FolderResolver]: Corvene's own storage, shared storage with
 * all-files access, or an imported copy of a Git repository), or null when
 * the user cancelled or the folder cannot be used (the reason is a toast).
 * Called exactly once per [FolderPicker.pick]. Without [import] (picking a
 * destination) folders are never copied.
 */
@Composable
fun rememberFolderPicker(import: Boolean = true, onResult: (String?) -> Unit): FolderPicker {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val callback = rememberUpdatedState(onResult)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree: Uri? ->
        if (tree == null) {
            callback.value(null)
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val picked = withContext(Dispatchers.IO) {
                FolderResolver.resolve(context, tree, import) { name ->
                    scope.launch { toast(context, context.getString(R.string.plt_importing, name)) }
                }
            }
            callback.value(picked.pathOrToast(context))
        }
    }
    return remember(launcher) {
        FolderPicker {
            try {
                launcher.launch(null)
            } catch (_: android.content.ActivityNotFoundException) {
                toast(context, context.getString(R.string.plt_no_folder_picker))
                callback.value(null)
            }
        }
    }
}

/**
 * The system file picker: the picked document is copied into the cache
 * ([FileCopier]) and [onResult] gets that copy's path, or null.
 */
@Composable
fun rememberFilePicker(onResult: (String?) -> Unit): FolderPicker {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val callback = rememberUpdatedState(onResult)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { document: Uri? ->
        if (document == null) {
            callback.value(null)
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val picked = withContext(Dispatchers.IO) { FileCopier.copy(context, document) }
            callback.value(picked.pathOrToast(context))
        }
    }
    return remember(launcher) {
        FolderPicker {
            try {
                launcher.launch(arrayOf("*/*"))
            } catch (_: android.content.ActivityNotFoundException) {
                toast(context, context.getString(R.string.plt_no_file_picker))
                callback.value(null)
            }
        }
    }
}

private fun Picked.pathOrToast(context: Context): String? = when (this) {
    is Picked.Path -> path
    is Picked.Failed -> {
        toast(context, message)
        null
    }
}

internal fun toast(context: Context, message: String) {
    Toast.makeText(context, message, Toast.LENGTH_LONG).show()
}
