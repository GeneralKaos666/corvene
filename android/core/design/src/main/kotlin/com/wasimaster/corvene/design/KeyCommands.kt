package com.wasimaster.corvene.design

import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * The hardware keyboard's commands that land inside a screen rather than in
 * the repository chrome (GHD's menu accelerators): the root's
 * `onPreviewKeyEvent` [send]s them, the screen that owns the thing collects
 * [commands] (the commit panel takes [KeyCommand.Commit], the changes list
 * [KeyCommand.Filter]).
 */
@Stable
class KeyCommands {
    private val flow = MutableSharedFlow<KeyCommand>(extraBufferCapacity = BUFFER)

    val commands: SharedFlow<KeyCommand> = flow.asSharedFlow()

    /** Whether a collector took it (false: nothing on screen handles it). */
    fun send(command: KeyCommand): Boolean = flow.subscriptionCount.value > 0 && flow.tryEmit(command)

    private companion object {
        const val BUFFER = 4
    }
}

/** What a shortcut asks of the screen under it. */
enum class KeyCommand {
    /** Ctrl+Enter: commit (opens the commit form first when it is closed). */
    Commit,

    /** Ctrl+F: show or hide the list's filter. */
    Filter,
}

/** The keyboard commands of the repository on screen; a fresh holder by default (nothing sends). */
val LocalKeyCommands = staticCompositionLocalOf { KeyCommands() }
