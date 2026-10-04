package com.wasimaster.corvene.platform

import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.wasimaster.corvene.common.CorveneLog

/**
 * A foreground service of type dataSync that runs while git talks to a
 * remote (the engine's `transfer_active`), so a long clone, fetch or push
 * goes on after the user leaves the app. It does no work itself: git runs
 * under the engine; the service only keeps the process. Its notification's
 * Stop ends the service (git goes on until Android stops the process; the
 * engine has no call that cancels every transfer yet).
 */
class TransferService : Service() {

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Notifications.createChannels(this)
        val stop = PendingIntent.getService(
            this,
            0,
            Intent(this, TransferService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val open = Notifications.launchIntent(this)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE)
        }
        val notification = NotificationCompat.Builder(this, Notifications.CHANNEL_TRANSFERS)
            .setSmallIcon(R.drawable.plt_ic_notification)
            .setContentTitle(getString(R.string.plt_transfer_running))
            .setOngoing(true)
            .setProgress(0, 0, true)
            .setContentIntent(open)
            .addAction(0, getString(R.string.plt_transfer_stop), stop)
            .build()
        // every start has to reach startForeground, also one that stops
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
        if (intent?.action == ACTION_STOP) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    /** Android 15 limits dataSync services to six hours a day. */
    override fun onTimeout(startId: Int, fgsType: Int) {
        CorveneLog.w("transfer service timed out")
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val ACTION_STOP = "com.wasimaster.corvene.STOP_TRANSFER"
        private const val NOTIFICATION_ID = 1
    }
}

/**
 * Starts [TransferService] once a transfer has run for [DELAY_MS] (short
 * operations end before it shows) and stops it when the engine says the last
 * one ended (CorveneActivity.transferActive of the GPUI app).
 */
object TransferController {
    private const val DELAY_MS = 1_500L
    private val main = Handler(Looper.getMainLooper())
    private var started = false
    private var pending: Runnable? = null

    fun transferActive(context: Context, active: Boolean) {
        val app = context.applicationContext
        main.post {
            pending?.let(main::removeCallbacks)
            pending = null
            if (active) {
                val start = Runnable { start(app) }
                pending = start
                main.postDelayed(start, DELAY_MS)
            } else {
                stop(app)
            }
        }
    }

    private fun start(context: Context) {
        if (started) return
        try {
            context.startForegroundService(Intent(context, TransferService::class.java))
            started = true
        } catch (error: IllegalStateException) {
            // not allowed from the background (Android 12+): the transfer goes on while the process lives
            CorveneLog.w("transfer service not started", error)
        } catch (error: SecurityException) {
            CorveneLog.w("transfer service not started", error)
        }
    }

    private fun stop(context: Context) {
        if (!started) return
        started = false
        context.stopService(Intent(context, TransferService::class.java))
    }
}
