package com.wasimaster.corvene.ffi

import android.content.Context
import com.wasimaster.corvene.common.CorveneLog
import com.wasimaster.corvene.common.CorveneTrace
import com.wasimaster.corvene.ffi.gen.CoreException
import com.wasimaster.corvene.ffi.gen.Corvene
import com.wasimaster.corvene.ffi.gen.HostInfo
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExecutorCoroutineDispatcher
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors

/** What the engine is started with (`Corvene::new`). */
data class CoreConfig(
    /** `Context.filesDir`: the store, git's HOME and the clones live under it. */
    val filesDir: String,
    /** `CORVENE_*` variables set before the engine reads them (`CORVENE_LOG`, `CORVENE_FLAGS`). */
    val env: Map<String, String>,
    val info: HostInfo,
)

/**
 * The engine, one per process. Rust owns the state; Kotlin asks for screen
 * view models ([query]) and sends actions ([dispatch]), both on one
 * dedicated thread so they reach the engine in order. After every change the
 * engine bumps [version] and screens query again ([rememberCoreQuery]).
 *
 * The engine starts on that thread as soon as the Core is created, so the
 * main thread never waits for the store to load; a query waits for it.
 */
class Core private constructor(context: Context, config: CoreConfig) {

    private val thread: ExecutorCoroutineDispatcher =
        Executors.newSingleThreadExecutor { Thread(it, THREAD_NAME) }.asCoroutineDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + thread)
    private val engine = CompletableDeferred<Corvene>()
    private val requests = Channel<HostRequest>(Channel.UNLIMITED)
    private val versionFlow = MutableStateFlow(0L)
    private val failure = MutableStateFlow<String?>(null)

    /** The engine's state version; changes whenever a screen should re-query. */
    val version: StateFlow<Long> = versionFlow.asStateFlow()

    /** Why the engine did not start, if it did not. */
    val startupFailure: StateFlow<String?> = failure.asStateFlow()

    /**
     * The engine's requests (pick a folder, open a URL, a toast, ...), queued
     * until the activity collects them: a request made while no activity is
     * resumed waits instead of being dropped.
     */
    val hostRequests: Flow<HostRequest> = requests.receiveAsFlow()

    init {
        scope.launch {
            CorveneTrace.section(CorveneTrace.ENGINE_INIT) {
                val started = System.nanoTime()
                try {
                    // the Keystore needs the context before the engine's first token read
                    Native.prepare(context)
                    val bridge = HostEventsBridge(
                        onStateChanged = { version ->
                            CorveneLog.d("state_changed $version")
                            versionFlow.value = version
                        },
                        send = { requests.trySend(it) },
                    )
                    engine.complete(Corvene(config.filesDir, config.env, config.info, bridge))
                    CorveneLog.i("engine started in ${(System.nanoTime() - started) / NANOS_PER_MILLI} ms")
                } catch (error: CoreException) {
                    fail("the engine did not start: ${error.message}", error)
                } catch (error: IllegalStateException) {
                    fail(error.message ?: "the engine did not start", error)
                }
            }
        }
    }

    private fun fail(message: String, error: Throwable) {
        CorveneLog.e(message, error)
        failure.value = message
        engine.completeExceptionally(error)
    }

    /** Runs [block] against the engine on its thread and returns the result. */
    suspend fun <T> query(block: suspend Corvene.() -> T): T = withContext(thread) {
        val corvene = engine.await()
        CorveneTrace.section(CorveneTrace.QUERY) { corvene.block() }
    }

    /** Sends an action; its effect comes back through [version] and a re-query. */
    fun dispatch(block: Corvene.() -> Unit) {
        scope.launch {
            val corvene = engine.await()
            CorveneTrace.section(CorveneTrace.DISPATCH) { corvene.block() }
        }
    }

    companion object {
        const val THREAD_NAME = "corvene-ffi"
        private const val NANOS_PER_MILLI = 1_000_000

        @Volatile
        private var instance: Core? = null

        /** The process's engine, started on first use. */
        fun start(context: Context, config: () -> CoreConfig): Core =
            instance ?: synchronized(this) {
                instance ?: Core(context.applicationContext, config()).also { instance = it }
            }

        /**
         * Whether this process runs the engine (the app was opened in it). A
         * process WorkManager started for the background fetch does not.
         */
        val isRunning: Boolean get() = instance != null

        /** The engine of this process, if it runs ([isRunning]). */
        val current: Core? get() = instance
    }
}
