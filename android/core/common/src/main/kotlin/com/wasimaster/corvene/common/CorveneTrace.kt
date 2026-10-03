package com.wasimaster.corvene.common

import androidx.tracing.trace

/**
 * The app's trace sections, all prefixed `Corvene:` so a Perfetto query finds
 * them together. Names are unique (the Konsist rules check both).
 */
object CorveneTrace {
    const val STARTUP = "Corvene:startup"
    const val ENGINE_INIT = "Corvene:engineInit"
    const val QUERY = "Corvene:query"
    const val DISPATCH = "Corvene:dispatch"
    const val HOST_REQUEST = "Corvene:hostRequest"

    inline fun <T> section(name: String, block: () -> T): T = trace(name, block)
}
