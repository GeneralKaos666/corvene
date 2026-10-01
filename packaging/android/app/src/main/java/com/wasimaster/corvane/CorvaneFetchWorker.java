package com.wasimaster.corvane;

import android.content.Context;

import androidx.work.Constraints;
import androidx.work.ExistingPeriodicWorkPolicy;
import androidx.work.NetworkType;
import androidx.work.PeriodicWorkRequest;
import androidx.work.WorkManager;
import androidx.work.Worker;
import androidx.work.WorkerParameters;

import java.util.concurrent.TimeUnit;

/**
 * The background fetch while Corvane is not on screen: its own timer stops
 * when Android freezes the process, so WorkManager wakes it about once an
 * hour, with a network. The fetch is the application's (the selected
 * repository, with the credentials of the signed-in accounts), which means
 * the process must still hold the activity; in a process started only for
 * this work there is nothing to ask and the work ends.
 */
public class CorvaneFetchWorker extends Worker {
    private static final String NAME = "background-fetch";

    public CorvaneFetchWorker(Context context, WorkerParameters parameters) {
        super(context, parameters);
    }

    static void schedule(Context context) {
        PeriodicWorkRequest request =
                new PeriodicWorkRequest.Builder(CorvaneFetchWorker.class, 1, TimeUnit.HOURS)
                        // the application fetches by itself while it is open
                        .setInitialDelay(1, TimeUnit.HOURS)
                        .setConstraints(new Constraints.Builder()
                                .setRequiredNetworkType(NetworkType.CONNECTED)
                                .build())
                        .build();
        WorkManager.getInstance(context.getApplicationContext())
                .enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request);
    }

    @Override
    public Result doWork() {
        if (!CorvaneActivity.isRunning() || !CorvaneActivity.nativeBackgroundFetch()) {
            return Result.success();
        }
        // the fetch starts on the native thread; stay until it has ended
        try {
            Thread.sleep(3000);
            for (int i = 0; i < 120 && CorvaneActivity.nativeNetworkBusy(); i++) {
                Thread.sleep(1000);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        return Result.success();
    }
}
