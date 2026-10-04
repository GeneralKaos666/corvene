package com.wasimaster.corvene.platform

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.produceState
import androidx.compose.ui.platform.LocalContext
import com.wasimaster.corvene.common.CorveneLog
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/**
 * An account's avatar as a file for `Avatar(path = …)` (Coil decodes files
 * only): downloaded once from [url] into `cache/avatars/`. The engine's view
 * models carry the URL, not a downloaded file yet (FFI-REQUESTS).
 */
@Composable
fun rememberAvatarPath(url: String?): State<String?> {
    val context = LocalContext.current
    return produceState<String?>(null, url) {
        value = url?.let { withContext(Dispatchers.IO) { AvatarCache.fetch(context, it) } }
    }
}

object AvatarCache {
    private const val LIMIT = 1L shl 20
    private const val TIMEOUT_MS = 10_000

    /** Blocking. The cached file, downloading it first; null for a non-https URL or a failure. */
    fun fetch(context: Context, url: String): String? {
        if (!url.startsWith("https://")) return null
        val folder = File(context.cacheDir, "avatars").apply { mkdirs() }
        val digest = MessageDigest.getInstance("SHA-256").digest(url.toByteArray()).joinToString("") { "%02x".format(it) }
        val file = File(folder, digest.take(32))
        if (file.isFile && file.length() > 0) return file.path
        val partial = File(folder, "${file.name}.part")
        return try {
            val connection = URL(url).openConnection() as HttpURLConnection
            connection.connectTimeout = TIMEOUT_MS
            connection.readTimeout = TIMEOUT_MS
            try {
                if (connection.responseCode != HttpURLConnection.HTTP_OK) return null
                connection.inputStream.use { input ->
                    partial.outputStream().use { output ->
                        val copied = input.copyTo(output)
                        if (copied > LIMIT) throw IOException("avatar too large")
                    }
                }
            } finally {
                connection.disconnect()
            }
            if (partial.renameTo(file)) file.path else null
        } catch (error: IOException) {
            CorveneLog.w("avatar download failed", error)
            partial.delete()
            null
        }
    }
}
