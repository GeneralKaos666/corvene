package com.wasimaster.corvene.ffi

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.ffi.gen.Corvene
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.collectLatest

/** The engine, for route composables. Screens take view models, never this. */
val LocalCore = staticCompositionLocalOf<Core> { error("no Core: CorveneApp provides it") }

/**
 * A query's latest result. [value] stays while a re-query runs
 * (stale-while-revalidate); [loading] is true until the first result.
 */
@Stable
data class QueryState<out T>(val value: T?, val loading: Boolean, val error: String?)

/**
 * Runs [query] against the engine now and after every state change, while the
 * caller's lifecycle is at least STARTED, and publishes the result only when it
 * differs from the last (UniFFI records are data classes, so `==` compares
 * contents and an unchanged screen does not recompose).
 */
@Composable
fun <T : Any> rememberCoreQuery(vararg keys: Any?, query: suspend Corvene.() -> T): State<QueryState<T>> {
    val core = LocalCore.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val latest = rememberUpdatedState(query)
    val state = remember(core, *keys) { mutableStateOf(QueryState<T>(value = null, loading = true, error = null)) }
    LaunchedEffect(core, lifecycle, *keys) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            core.version.collectLatest {
                val next = try {
                    QueryState(core.query { latest.value(this) }, loading = false, error = null)
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (failure: Exception) {
                    CorveneLog.w("query failed", failure)
                    state.value.copy(loading = false, error = failure.message ?: failure.javaClass.simpleName)
                }
                if (next != state.value) state.value = next
            }
        }
    }
    return state
}
