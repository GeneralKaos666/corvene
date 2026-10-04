package com.wasimaster.corvene.settings

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery

/** Settings › Appearance wired to the engine: `settings()` in, `setDesignStyle` / `setTheme` out. */
@Composable
fun AppearanceRoute(modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val settings by rememberCoreQuery { settings() }
    val value = settings.value ?: return
    AppearanceScreen(
        settings = value,
        dark = CorveneTheme.colors.isDark,
        onStyle = { style -> core.dispatch { setDesignStyle(style) } },
        onTheme = { theme -> core.dispatch { setTheme(theme) } },
        modifier = modifier,
        contentPadding = contentPadding,
    )
}
