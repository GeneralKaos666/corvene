package com.wasimaster.corvene.platform

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.work.NetworkType
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.testing.WorkManagerTestInitHelper
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit

/** The hourly fetch is one unique periodic work with its constraints; scheduling again keeps one. */
@RunWith(AndroidJUnit4::class)
class CorveneFetchWorkerTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun init() = WorkManagerTestInitHelper.initializeTestWorkManager(context)

    private fun infos(): List<WorkInfo> = WorkManager.getInstance(context).getWorkInfosForUniqueWork(CorveneFetchWorker.NAME).get()

    @Test
    fun `scheduled hourly with a network and a battery that is not low`() {
        CorveneFetchWorker.schedule(context)
        val info = infos().single()
        assertEquals(WorkInfo.State.ENQUEUED, info.state)
        assertEquals(NetworkType.CONNECTED, info.constraints.requiredNetworkType)
        assertTrue(info.constraints.requiresBatteryNotLow())
        assertEquals(TimeUnit.HOURS.toMillis(1), info.periodicityInfo?.repeatIntervalMillis)
    }

    @Test
    fun `scheduling again keeps one`() {
        CorveneFetchWorker.schedule(context)
        val first = infos().single().id
        CorveneFetchWorker.schedule(context)
        assertEquals(first, infos().single().id)
    }
}
