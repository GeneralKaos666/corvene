package com.wasimaster.corvene.platform

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context

/**
 * Puts [text] on the system clipboard (the engine's `write_clipboard`
 * request, and Kotlin's own Copy actions such as History › Copy SHA).
 */
fun writeClipboard(context: Context, text: String) {
    val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return
    clipboard.setPrimaryClip(ClipData.newPlainText("Corvene", text))
}
