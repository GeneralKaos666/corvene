package com.wasimaster.corvene

import android.content.Intent
import android.os.Bundle
import android.view.KeyboardShortcutGroup
import android.view.Menu
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.lifecycleScope
import androidx.metrics.performance.JankStats
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.Headless
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.CoreException
import com.wasimaster.corvene.platform.AppLinks
import com.wasimaster.corvene.platform.HostState
import com.wasimaster.corvene.platform.Notifications
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The one activity. Shows the splash screen until the engine has answered its
 * first query (or failed, or [SPLASH_TIMEOUT_MS] passed), then the app edge to
 * edge. Hands links to the engine (`x-corvene://`, the browser sign-in's
 * `x-corvene-auth://` callback, a shared repository address, see
 * [AppLinks]) and notification taps; [CorveneApp] tells it when the app is
 * visible and focused. singleTask: a Custom Tab over it returns here.
 */
class MainActivity : ComponentActivity() {

    private val core get() = (application as CorveneApp).core

    @Volatile
    private var ready = false

    private var jank: JankStats? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        // a background fetch WorkManager started in this process lets go
        // before the engine starts (headless and app never run together)
        Headless.end()
        // the engine starts loading the store on its thread while the activity inflates
        core
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        splash.setKeepOnScreenCondition { !ready }
        lifecycleScope.launch {
            withTimeoutOrNull(SPLASH_TIMEOUT_MS) {
                // a failed start shows on the list screen; the splash only waits for it
                try {
                    core.query { settings() to repoList() }
                } catch (_: CoreException) {
                    Unit
                } catch (_: IllegalStateException) {
                    Unit
                }
            }
            ready = true
        }
        if (savedInstanceState == null) handleIntent(intent)
        jank = JankMonitor.install(this)
        setContent {
            CompositionLocalProvider(LocalCore provides core) {
                CorveneRoot(
                    onQuit = { finishAffinity() },
                    // a flag that needs a restart
                    onRelaunch = { RelaunchActivity.relaunch(this) },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // all-files access or notifications may have been granted in Android's settings meanwhile
        HostState.refresh(core, this)
        jank?.isTrackingEnabled = true
    }

    override fun onPause() {
        jank?.isTrackingEnabled = false
        super.onPause()
    }

    /** The hardware keyboard's shortcuts in the system's list (Meta+/), as GHD's menus show them. */
    override fun onProvideKeyboardShortcuts(data: MutableList<KeyboardShortcutGroup>, menu: Menu?, deviceId: Int) {
        super.onProvideKeyboardShortcuts(data, menu, deviceId)
        data += KeyboardShortcuts.groups(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        if (intent.action == Notifications.ACTION_NOTIFICATION) {
            val identifier = intent.getStringExtra(Notifications.EXTRA_IDENTIFIER).orEmpty()
            val payload = intent.getStringExtra(Notifications.EXTRA_PAYLOAD)
            core.dispatch { notificationClicked(identifier, payload) }
            return
        }
        AppLinks.appUrl(intent)?.let { url ->
            CorveneLog.i("app url: ${url.substringBefore('?')}")
            core.dispatch { appUrl(url) }
        }
        // Debug builds only: add a repository by path without the picker, for
        // scripted tests (`am start ... --es corvene.debug.addRepository <path>`).
        if (BuildConfig.DEBUG) {
            intent.getStringExtra(DEBUG_ADD_REPOSITORY)?.let { path ->
                CorveneLog.i("debug: adding $path")
                core.dispatch { addRepository(path) }
            }
        }
    }

    private companion object {
        const val SPLASH_TIMEOUT_MS = 3_000L
        const val DEBUG_ADD_REPOSITORY = "corvene.debug.addRepository"
    }
}
