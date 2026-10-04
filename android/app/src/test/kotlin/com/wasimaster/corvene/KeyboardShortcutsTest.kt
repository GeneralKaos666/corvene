package com.wasimaster.corvene

import android.app.Application
import android.view.KeyEvent
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

/** GHD's accelerators (⌘ → Ctrl) map to [Shortcut]s, and the system list carries them all. */
@RunWith(AndroidJUnit4::class)
@Config(application = Application::class)
class KeyboardShortcutsTest {

    private fun press(code: Int, meta: Int = 0, action: Int = KeyEvent.ACTION_DOWN) =
        Shortcut.of(androidx.compose.ui.input.key.KeyEvent(KeyEvent(0, 0, action, code, 0, meta)))

    @Test
    fun `ctrl and shift pick the shortcut`() {
        assertEquals(Shortcut.Changes, press(KeyEvent.KEYCODE_1, KeyEvent.META_CTRL_ON))
        assertEquals(Shortcut.History, press(KeyEvent.KEYCODE_2, KeyEvent.META_CTRL_ON))
        assertEquals(Shortcut.Push, press(KeyEvent.KEYCODE_P, KeyEvent.META_CTRL_ON))
        assertEquals(Shortcut.Pull, press(KeyEvent.KEYCODE_P, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON))
        assertEquals(Shortcut.Fetch, press(KeyEvent.KEYCODE_T, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON))
        assertEquals(Shortcut.Repositories, press(KeyEvent.KEYCODE_T, KeyEvent.META_CTRL_ON))
        assertEquals(Shortcut.NewBranch, press(KeyEvent.KEYCODE_N, KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON))
        assertEquals(Shortcut.Commit, press(KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.META_CTRL_ON))
        assertEquals(Shortcut.Close, press(KeyEvent.KEYCODE_ESCAPE))
    }

    @Test
    fun `plain keys, key ups and alt are not shortcuts`() {
        assertNull(press(KeyEvent.KEYCODE_1))
        assertNull(press(KeyEvent.KEYCODE_1, KeyEvent.META_CTRL_ON, KeyEvent.ACTION_UP))
        assertNull(press(KeyEvent.KEYCODE_1, KeyEvent.META_CTRL_ON or KeyEvent.META_ALT_ON))
    }

    @Test
    fun `the system list has every shortcut in three groups`() {
        val groups = KeyboardShortcuts.groups(ApplicationProvider.getApplicationContext())
        assertEquals(3, groups.size)
        assertEquals(Shortcut.entries.size, groups.sumOf { it.items.size })
    }
}
