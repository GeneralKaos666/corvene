package com.wasimaster.corvene

import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

/**
 * Navigation 3 keys. Serializable, so the back stack survives process death;
 * the rest of the state lives in the engine's store.
 */
@Serializable
data object Repositories : NavKey

/** A repository's screens: Changes and History under the repository chrome. */
@Serializable
data class Repository(val id: Long) : NavKey

/** The diff of the selected file on its own screen (compact widths; wider ones show it beside the list). */
@Serializable
data class Diff(val id: Long) : NavKey

/** Settings › Appearance. */
@Serializable
data object AppearanceSettings : NavKey
