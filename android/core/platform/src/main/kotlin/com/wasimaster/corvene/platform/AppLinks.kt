package com.wasimaster.corvene.platform

import android.content.Intent

/**
 * What an intent that opened the app asks the engine (`appUrl`): an
 * `x-corvene://` / `x-corvene-auth://` link as it is, a GitHub link turned
 * into `x-corvene://openRepo/<url>`, and shared text (ACTION_SEND) holding a
 * repository's address (https, ssh, git or `user@host:path`) likewise, which
 * makes the engine offer to clone it. Null when there is nothing for it.
 */
object AppLinks {
    private val REMOTE = Regex("(?i)(https?|ssh|git)://[^/\\s]+/\\S+")
    private val SCP = Regex("[\\w.-]+@[\\w.-]+:\\S+")

    fun appUrl(action: String?, data: String?, text: String?): String? = when (action) {
        Intent.ACTION_VIEW -> data?.let(::fromLink)
        Intent.ACTION_SEND -> text?.let(::fromText)
        else -> null
    }

    fun appUrl(intent: Intent): String? = appUrl(intent.action, intent.dataString, intent.getStringExtra(Intent.EXTRA_TEXT))

    private fun fromLink(url: String): String? = when {
        url.startsWith("x-corvene://", ignoreCase = true) || url.startsWith("x-corvene-auth://", ignoreCase = true) -> url
        REMOTE.matches(url) -> "x-corvene://openRepo/$url"
        else -> null
    }

    /** The first word that is a repository's address (CorveneActivity.openRepositoryAddress). */
    fun fromText(text: String): String? = text.trim().split(Regex("\\s+"))
        .firstOrNull { REMOTE.matches(it) || SCP.matches(it) }
        ?.let { "x-corvene://openRepo/$it" }
}
