package com.wasimaster.corvene

import android.graphics.Color
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.LocalActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.HostRequestHandler

/**
 * The app under the activity: the engine's appearance settings decide the
 * theme (the splash screen stays up until they have been read once), the
 * system bars follow light/dark, and the engine's host requests are served.
 */
@Composable
fun CorveneRoot(onQuit: () -> Unit) {
    val core = LocalCore.current
    val settings by rememberCoreQuery { settings() }
    val appearance = settings.value?.toAppearance() ?: return
    val dark = when (appearance.colorMode) {
        ColorMode.System -> isSystemInDarkTheme()
        ColorMode.Light -> false
        ColorMode.Dark -> true
    }
    val activity = LocalActivity.current as? ComponentActivity
    LaunchedEffect(activity, dark) {
        val bars = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark }
        activity?.enableEdgeToEdge(statusBarStyle = bars, navigationBarStyle = bars)
    }
    CorveneTheme(style = appearance.style, colorMode = appearance.colorMode, highContrast = appearance.highContrast) {
        HostRequestHandler(core, onQuit = onQuit)
        CorveneNavigation(
            appearance = appearance,
            onStyle = { style -> core.dispatch { setDesignStyle(style.toVm()) } },
            onTheme = { theme -> core.dispatch { setTheme(theme) } },
        )
    }
}
