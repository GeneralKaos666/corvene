package com.wasimaster.corvene.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.structuralEqualityPolicy

/**
 * A value replaced as a whole but read a piece at a time (WMKeyboard's
 * pattern): a screen's view model, re-queried from the engine after every
 * state change, read by rows that each care about one field of it.
 *
 * The holder stays the same instance, so nothing that takes it recomposes on
 * a new value; [watch] subscribes only its caller's scope, and only to the
 * piece it selects (compared by `equals`). In callbacks read [value].
 */
@Stable
class LiveState<T>(private val state: State<T>) {

    /** The whole value, now. For callbacks and effects. */
    val value: T get() = state.value

    /** The piece [select] picks, recomposing the caller only when that piece changes. */
    @Composable
    fun <R> watch(select: (T) -> R): R {
        val selector = rememberUpdatedState(select)
        return remember(this) {
            derivedStateOf(structuralEqualityPolicy()) { selector.value(state.value) }
        }.value
    }
}
