package com.wasimaster.corvene

import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

/**
 * Navigation 3 keys. Serializable, so the back stack survives process death;
 * the rest of the state lives in the engine's store.
 */
@Serializable
data object Repositories : NavKey

/** A repository's screens (Changes, History, Branches from M-A1). */
@Serializable
data class Repository(val id: Long) : NavKey
