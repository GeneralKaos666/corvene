package com.wasimaster.corvene.platform

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.Headless
import com.wasimaster.corvene.ffi.gen.HeadlessOutcome
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.TimeUnit

/**
 * The background fetch while Corvene is not on screen: the engine's own timer
 * stops when Android freezes the process, so WorkManager runs this about
 * once an hour with a network. In a process WorkManager started for it (no
 * engine) the engine fetches the selected repository headless, with the
 * credentials in the Keystore. When the app runs in this process the tick
 * goes to the live engine (`backgroundFetch`, the fetcher's own rules), which
 * runs it on its threads while the process lives.
 */
class CorveneFetchWorker(context: Context, parameters: WorkerParameters) : CoroutineWorker(context, parameters) {

    override suspend fun doWork(): Result {
        Core.current?.let { core ->
            CorveneLog.i("background fetch: the app runs in this process, asking its engine")
            core.dispatch { backgroundFetch() }
            return Result.success()
        }
        val outcome = withContext(Dispatchers.IO) { Headless.fetch(applicationContext) }
        CorveneLog.i("background fetch (headless): $outcome")
        return if (outcome == HeadlessOutcome.FAILED) Result.retry() else Result.success()
    }

    companion object {
        const val NAME = "background-fetch"

        /**
         * Hourly with a network and a battery that is not low. KEEP: every launch
         * calls this, and UPDATE would push the first run an hour out each time, so
         * the fetch never ran while the app was in daily use.
         */
        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<CorveneFetchWorker>(1, TimeUnit.HOURS)
                // the app fetches by itself while it is open
                .setInitialDelay(1, TimeUnit.HOURS)
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .setRequiresBatteryNotLow(true)
                        .build(),
                )
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }
    }
}
