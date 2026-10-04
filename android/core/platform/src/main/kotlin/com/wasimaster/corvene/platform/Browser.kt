package com.wasimaster.corvene.platform

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.provider.Settings
import androidx.browser.customtabs.CustomTabColorSchemeParams
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.net.toUri

/** Opens [url] in the app that handles it (the browser), or says there is none. */
fun openUrl(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        toast(context, context.getString(R.string.plt_no_app_for_url, url))
    }
}

/**
 * A page in a Custom Tab: the browser's tab drawn over Corvene in its task,
 * so signing in does not leave the app and `x-corvene-auth://` returns to it.
 * A browser without Custom Tabs opens the page normally.
 */
fun openCustomTab(context: Context, url: String, toolbarColor: Int = GITHUB_DARK) {
    val intent = CustomTabsIntent.Builder()
        .setDefaultColorSchemeParams(CustomTabColorSchemeParams.Builder().setToolbarColor(toolbarColor).build())
        .setShowTitle(true)
        .build()
    if (context !is Activity) intent.intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        intent.launchUrl(context, url.toUri())
    } catch (_: ActivityNotFoundException) {
        toast(context, context.getString(R.string.plt_no_browser))
    }
}

/** GitHub's sign-in pages (OAuth authorize, device code) go to a Custom Tab; everything else to the browser. */
fun isSignInUrl(url: String): Boolean {
    val uri = url.toUri()
    val path = uri.path.orEmpty()
    return uri.scheme == "https" && (path.startsWith("/login/oauth/authorize") || path.startsWith("/login/device"))
}

/** The system's "All files access" page for this app (foss, Android 11+). */
fun requestAllFilesAccess(context: Context) {
    val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, "package:${context.packageName}".toUri())
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        try {
            context.startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
        } catch (_: ActivityNotFoundException) {
            toast(context, context.getString(R.string.plt_no_all_files_settings))
        }
    }
}

/**
 * Back to the app from a tab opened over it (the sign-in finished): the
 * activity is singleTask, so starting it again closes what is above it.
 */
fun bringToFront(context: Context) {
    val intent = Notifications.launchIntent(context) ?: return
    intent.addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
    if (context !is Activity) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        // the page says to return to Corvene
    }
}

private const val GITHUB_DARK = 0xff24292e.toInt()
