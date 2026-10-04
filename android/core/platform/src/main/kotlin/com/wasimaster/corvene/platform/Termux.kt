package com.wasimaster.corvene.platform

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Environment
import android.os.Handler
import android.os.Looper
import com.wasimaster.corvene.common.CorveneLog
import java.io.File

/**
 * A Termux session in a repository through Termux's RUN_COMMAND intent
 * (https://github.com/termux/termux-app/wiki/RUN_COMMAND-Intent): needs its
 * runtime permission (asked for here) and `allow-external-apps = true` in
 * ~/.termux/termux.properties. Termux cannot read Corvene's private storage,
 * so only repositories on shared storage open. The editors probe
 * (CorveneActivity.termuxEditors) arrives with Settings › Integrations.
 */
object Termux {
    const val PACKAGE = "com.termux"
    const val PERMISSION = "com.termux.permission.RUN_COMMAND"
    private const val REQUEST_PERMISSION = 3
    private const val LOGIN = "/data/data/com.termux/files/usr/bin/login"

    fun installed(context: Context): Boolean = try {
        context.packageManager.getPackageInfo(PACKAGE, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }

    /** Opens a session in [directory]; returns an error for the user, or null when the intent went. */
    private const val SERVICE_DELAY_MS = 800L

    fun open(activity: Activity, directory: String): String? {
        if (!installed(activity)) return activity.getString(R.string.plt_termux_missing)
        @Suppress("DEPRECATION") // Termux needs the path
        val shared = Environment.getExternalStorageDirectory().absolutePath
        val path = File(directory).absolutePath
        if (path != shared && !path.startsWith("$shared/")) return activity.getString(R.string.plt_termux_private)
        if (activity.checkSelfPermission(PERMISSION) != PackageManager.PERMISSION_GRANTED) {
            activity.requestPermissions(arrayOf(PERMISSION), REQUEST_PERMISSION)
            return activity.getString(R.string.plt_termux_permission)
        }
        val intent = Intent("com.termux.RUN_COMMAND")
            .setClassName(PACKAGE, "com.termux.app.RunCommandService")
            .putExtra("com.termux.RUN_COMMAND_PATH", LOGIN)
            .putExtra("com.termux.RUN_COMMAND_WORKDIR", path)
            .putExtra("com.termux.RUN_COMMAND_BACKGROUND", false)
            // switch to the new session and open Termux
            .putExtra("com.termux.RUN_COMMAND_SESSION_ACTION", "0")
        return try {
            // A Termux the user restricted in the background (ColorOS does by
            // default) rejects the service while it is not in the foreground:
            // its activity first, the command once it is up.
            activity.startActivity(
                Intent().setClassName(PACKAGE, "com.termux.app.TermuxActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            Handler(Looper.getMainLooper()).postDelayed({
                try {
                    activity.startService(intent)
                } catch (error: RuntimeException) {
                    CorveneLog.w("termux command not started: ${error.message.orEmpty()}")
                }
            }, SERVICE_DELAY_MS)
            null
        } catch (error: SecurityException) {
            activity.getString(R.string.plt_termux_failed, error.message.orEmpty())
        } catch (error: IllegalStateException) {
            activity.getString(R.string.plt_termux_failed, error.message.orEmpty())
        }
    }
}
