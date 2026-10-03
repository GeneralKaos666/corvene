package com.wasimaster.corvene;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;

/**
 * A foreground service of type dataSync that runs while git talks to a
 * remote (CorveneActivity.transferActive), so a long clone, fetch or push
 * goes on after the user leaves the application. It does no work itself:
 * git runs under the native code, the service only keeps the process.
 */
public class CorveneTransferService extends Service {
    static final String ACTION_STOP = "com.wasimaster.corvene.STOP_TRANSFER";
    private static final String CHANNEL = "transfers";
    private static final int NOTIFICATION_ID = 1;

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager manager = getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL,
                getString(R.string.channel_transfers), NotificationManager.IMPORTANCE_LOW));
        Notification notification = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(getString(R.string.transfer_running))
                .setOngoing(true)
                .build();
        // every start has to reach startForeground, also one that stops
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            stopForeground(true);
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    /** Android 15 limits dataSync services to six hours a day. */
    @Override
    public void onTimeout(int startId, int fgsType) {
        stopForeground(true);
        stopSelf();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
