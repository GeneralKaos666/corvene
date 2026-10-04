package com.wasimaster.corvene.platform

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * The system notification channels ("Pull requests" for the engine's
 * `show_notification`, "Network operations" for [TransferService]) and the
 * pull request notifications. Tapping one opens the app with
 * [ACTION_NOTIFICATION] and the identifier / payload extras.
 */
object Notifications {
    const val CHANNEL_PULL_REQUESTS = "pull-requests"
    const val CHANNEL_TRANSFERS = "transfers"
    const val ACTION_NOTIFICATION = "com.wasimaster.corvene.NOTIFICATION"
    const val EXTRA_IDENTIFIER = "identifier"
    const val EXTRA_PAYLOAD = "payload"

    /** Creates both channels (idempotent); at process start. */
    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannels(
            listOf(
                NotificationChannel(
                    CHANNEL_PULL_REQUESTS,
                    context.getString(R.string.plt_channel_pull_requests),
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
                NotificationChannel(
                    CHANNEL_TRANSFERS,
                    context.getString(R.string.plt_channel_transfers),
                    NotificationManager.IMPORTANCE_LOW,
                ),
            ),
        )
    }

    /** Whether the app may post (POST_NOTIFICATIONS on 13+, and not blocked in Settings). */
    fun allowed(context: Context): Boolean {
        val granted = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        return granted && NotificationManagerCompat.from(context).areNotificationsEnabled()
    }

    /** A pull request notification; tapping it opens the app with [payload]. */
    fun show(context: Context, identifier: String, title: String, body: String, payload: String) {
        if (!allowed(context)) return
        val open = launchIntent(context)
            ?.setAction(ACTION_NOTIFICATION)
            ?.putExtra(EXTRA_IDENTIFIER, identifier)
            ?.putExtra(EXTRA_PAYLOAD, payload)
            ?.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            ?: return
        val pending = PendingIntent.getActivity(
            context,
            identifier.hashCode(),
            open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL_PULL_REQUESTS)
            .setSmallIcon(R.drawable.plt_ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(pending)
            .setAutoCancel(true)
            .build()
        try {
            NotificationManagerCompat.from(context).notify(identifier, 0, notification)
        } catch (_: SecurityException) {
            // the permission was revoked between the check and the post
        }
    }

    /** The launcher activity as an explicit intent (this module does not know its class). */
    fun launchIntent(context: Context): Intent? =
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.component?.let { Intent().setComponent(it) }
}
