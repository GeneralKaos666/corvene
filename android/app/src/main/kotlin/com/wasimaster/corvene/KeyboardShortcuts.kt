package com.wasimaster.corvene

import android.content.Context
import android.view.KeyEvent
import android.view.KeyboardShortcutGroup
import android.view.KeyboardShortcutInfo
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.type

/**
 * GHD's menu accelerators on a hardware keyboard (⌘ → Ctrl): View ›
 * Show Changes ⌘1 / History ⌘2 / Repository list ⌘T / Branches ⌘B, Find
 * ⌘F; Repository › Push ⌘P / Pull ⇧⌘P / Fetch ⇧⌘T; Branch › New Branch
 * ⇧⌘N; ⌘Enter commits; Esc closes the open sheet.
 */
enum class Shortcut(val keyCode: Int, val ctrl: Boolean, val shift: Boolean, val label: Int, val group: Group) {
    Changes(KeyEvent.KEYCODE_1, true, false, R.string.app_shortcut_changes, Group.View),
    History(KeyEvent.KEYCODE_2, true, false, R.string.app_shortcut_history, Group.View),
    Repositories(KeyEvent.KEYCODE_T, true, false, R.string.app_shortcut_repositories, Group.View),
    Branches(KeyEvent.KEYCODE_B, true, false, R.string.app_shortcut_branches, Group.View),
    Filter(KeyEvent.KEYCODE_F, true, false, R.string.app_shortcut_filter, Group.View),
    Close(KeyEvent.KEYCODE_ESCAPE, false, false, R.string.app_shortcut_close, Group.View),
    Commit(KeyEvent.KEYCODE_ENTER, true, false, R.string.app_shortcut_commit, Group.Repository),
    Push(KeyEvent.KEYCODE_P, true, false, R.string.app_shortcut_push, Group.Repository),
    Pull(KeyEvent.KEYCODE_P, true, true, R.string.app_shortcut_pull, Group.Repository),
    Fetch(KeyEvent.KEYCODE_T, true, true, R.string.app_shortcut_fetch, Group.Repository),
    NewBranch(KeyEvent.KEYCODE_N, true, true, R.string.app_shortcut_new_branch, Group.Branch),
    ;

    enum class Group(val label: Int) {
        View(R.string.app_shortcuts_view),
        Repository(R.string.app_shortcuts_repository),
        Branch(R.string.app_shortcuts_branch),
    }

    companion object {
        /** The shortcut a key press is, if any (key down only; Alt and Meta never). */
        fun of(event: androidx.compose.ui.input.key.KeyEvent): Shortcut? {
            if (event.type != KeyEventType.KeyDown || event.isAltPressed || event.isMetaPressed) return null
            val code = event.key.nativeKeyCode
            val enter = code == KeyEvent.KEYCODE_NUMPAD_ENTER
            return entries.firstOrNull {
                (it.keyCode == code || (enter && it.keyCode == KeyEvent.KEYCODE_ENTER)) &&
                    it.ctrl == event.isCtrlPressed &&
                    it.shift == event.isShiftPressed
            }
        }
    }
}

/** [Shortcut]s for Android's keyboard shortcuts list (`Activity.onProvideKeyboardShortcuts`). */
object KeyboardShortcuts {
    fun groups(context: Context): List<KeyboardShortcutGroup> = Shortcut.Group.entries.map { group ->
        KeyboardShortcutGroup(
            context.getString(group.label),
            Shortcut.entries.filter { it.group == group }.map { shortcut ->
                var modifiers = 0
                if (shortcut.ctrl) modifiers = modifiers or KeyEvent.META_CTRL_ON
                if (shortcut.shift) modifiers = modifiers or KeyEvent.META_SHIFT_ON
                KeyboardShortcutInfo(context.getString(shortcut.label), shortcut.keyCode, modifiers)
            },
        )
    }
}
