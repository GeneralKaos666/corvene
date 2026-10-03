package com.wasimaster.corvene

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.lifecycleScope
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.CoreException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The one activity. Shows the splash screen until the engine has answered its
 * first query (or failed, or [SPLASH_TIMEOUT_MS] passed), then the app edge to
 * edge. Hands `x-corvene://` links to the engine and tells it when the app
 * comes to the front (GHD refreshes on window focus).
 */
class MainActivity : ComponentActivity() {

    private val core get() = (application as CorveneApp).core

    @Volatile
    private var ready = false

    override fun onCreate(savedInstanceState: Bundle?) {
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
        if (savedInstanceState == null) {
            // GHD's indicator updater runs at start-up; the engine leaves it to the host (FFI-REQUESTS.md #9)
            core.dispatch { refreshIndicators() }
            handleIntent(intent)
        }
        setContent {
            CompositionLocalProvider(LocalCore provides core) {
                CorveneRoot(onQuit = ::finishAndRemoveTask)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        core.dispatch { focus() }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        val url = intent.dataString
        if (intent.action == Intent.ACTION_VIEW && url != null) core.dispatch { appUrl(url) }
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
