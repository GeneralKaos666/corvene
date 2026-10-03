package com.wasimaster.corvene.platform

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

/** Opens the system folder picker; see [rememberFolderPicker]. */
fun interface FolderPicker {
    fun pick()
}

/**
 * The system folder picker (Storage Access Framework). [onResult] gets the
 * resolved path, or null when the user cancelled or the folder cannot be used
 * in place (the reason is shown as a toast).
 */
@Composable
fun rememberFolderPicker(onResult: (String?) -> Unit): FolderPicker {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val callback = rememberUpdatedState(onResult)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree: Uri? ->
        if (tree == null) {
            callback.value(null)
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val picked = withContext(Dispatchers.IO) { FolderResolver.resolve(context, tree) }
            when (picked) {
                is PickedFolder.Path -> callback.value(picked.path)
                is PickedFolder.Unsupported -> {
                    Toast.makeText(context, picked.message, Toast.LENGTH_LONG).show()
                    callback.value(null)
                }
            }
        }
    }
    return remember(launcher) { FolderPicker { launcher.launch(null) } }
}
