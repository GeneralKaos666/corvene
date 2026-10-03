package com.wasimaster.corvene.design

/**
 * How the app looks. One code base, three skins over the same components:
 *
 * - [GitHubMobile]: GitHub for Android's look (Primer primitives, Inter, bottom
 *   navigation). The default on Android.
 * - [GitHubDesktop]: GitHub Desktop 3.6.6's tokens and type sizes, its toolbar
 *   and tab bar.
 * - [Material]: Material 3 with dynamic colour (Android 12+), Primer's
 *   semantic hues kept for states and diffs.
 *
 * Persisted by the engine's settings (`Settings.design_style`, flag
 * `112-design-style` pins it); :app maps the engine's `DesignStyleVm` to this.
 */
enum class DesignStyle(val key: String) {
    GitHubMobile("github-mobile"),
    GitHubDesktop("github-desktop"),
    Material("material"),
    ;

    companion object {
        val Default = GitHubMobile

        fun fromKey(key: String?): DesignStyle = entries.firstOrNull { it.key == key } ?: Default
    }
}

/** Light, dark, or whatever the system says. */
enum class ColorMode(val key: String) {
    System("system"),
    Light("light"),
    Dark("dark"),
    ;

    companion object {
        fun fromKey(key: String?): ColorMode = entries.firstOrNull { it.key == key } ?: System
    }
}
