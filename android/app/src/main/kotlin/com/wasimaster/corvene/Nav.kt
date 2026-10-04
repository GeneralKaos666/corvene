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

/** The selected commit on its own screen (compact widths; wider ones show it beside History). */
@Serializable
data class CommitDetail(val id: Long) : NavKey

/** The selected commit's selected file (compact widths). */
@Serializable
data class CommitDiff(val id: Long) : NavKey

/** Clone a repository (full screen); [url] prefilled from a link or shared text. */
@Serializable
data class Clone(val url: String? = null) : NavKey

/** Add an existing repository (full screen). */
@Serializable
data object AddRepository : NavKey

/** Create a new repository (full screen). */
@Serializable
data object CreateRepository : NavKey

/** Sign in on its own (Settings › Accounts, the Clone dialog). */
@Serializable
data class SignIn(val enterprise: Boolean) : NavKey

/** Settings › Accounts. */
@Serializable
data object AccountsSettings : NavKey
