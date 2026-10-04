package com.wasimaster.corvene

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.os.Process
import com.wasimaster.corvene.common.CorveneLog

/**
 * Starts Corvene again in a fresh process (the engine's `relaunch()`, for
 * flags read once at start): ProcessPhoenix's approach. The app's process
 * opens this activity in the `:relaunch` process and exits; this one makes
 * sure the old process is gone, starts [MainActivity] as a new task and
 * exits too. No window is drawn.
 */
class RelaunchActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pid = intent.getIntExtra(EXTRA_PID, -1)
        if (pid > 0 && pid != Process.myPid()) Process.killProcess(pid)
        startActivity(
            Intent(this, MainActivity::class.java)
                .setAction(Intent.ACTION_MAIN)
                .addCategory(Intent.CATEGORY_LAUNCHER)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK),
        )
        finish()
        Process.killProcess(Process.myPid())
    }

    companion object {
        private const val EXTRA_PID = "corvene.relaunch.pid"

        /** From the app's process: hand over to [RelaunchActivity] and end this process (the engine has saved by now). */
        fun relaunch(activity: Activity) {
            CorveneLog.i("relaunching")
            activity.startActivity(
                Intent(activity, RelaunchActivity::class.java)
                    .putExtra(EXTRA_PID, Process.myPid())
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            activity.finishAffinity()
            Process.killProcess(Process.myPid())
        }
    }
}
