package com.wasimaster.corvene.benchmark

import android.view.KeyEvent
import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.util.regex.Pattern

/** The app under test: the non-debuggable variants have no applicationId suffix. */
const val TARGET = "com.wasimaster.corvene"

private const val TIMEOUT_MS = 5_000L
private const val DEFAULT_REPO = "/sdcard/Corvene/bench"

/** Test tags are resource ids (CorveneRoot sets `testTagsAsResourceId`). */
private fun tag(regex: String): Pattern = Pattern.compile(regex)

private fun MacrobenchmarkScope.find(regex: String): UiObject2? =
    device.wait(Until.findObject(By.res(tag(regex))), TIMEOUT_MS)

/**
 * The repository list, then the first repository. A fresh install has
 * none: the repository at the `corvene.benchRepo` instrumentation argument
 * (default /sdcard/Corvene/bench, readable with All files access) is added
 * through `x-corvene://openLocalRepo` first.
 */
fun MacrobenchmarkScope.openFirstRepository() {
    var row = find("repo_row_.*")
    if (row == null) {
        val path = InstrumentationRegistry.getArguments().getString("corvene.benchRepo") ?: DEFAULT_REPO
        device.executeShellCommand("am start -a android.intent.action.VIEW -d x-corvene://openLocalRepo$path -p $TARGET")
        device.waitForIdle()
        row = find("repo_row_.*")
    }
    row?.click()
    device.wait(Until.hasObject(By.res(tag("chg_.*"))), TIMEOUT_MS)
}

/** Changes: open the first file's diff, scroll it down and back. */
fun MacrobenchmarkScope.scrollFirstDiff() {
    find("chg_file_.*")?.click() ?: return
    val diff = find("chg_diff_list") ?: return
    diff.setGestureMargin(device.displayWidth / GESTURE_MARGIN)
    repeat(FLINGS) { diff.fling(Direction.DOWN) }
    repeat(FLINGS) { diff.fling(Direction.UP) }
    // back to the list on compact widths (the diff is its own screen there)
    if (find("chg_files") == null) device.pressBack()
}

/** History (Ctrl+2): scroll the commits down and back. */
fun MacrobenchmarkScope.scrollHistory() {
    device.pressKeyCode(KeyEvent.KEYCODE_2, KeyEvent.META_CTRL_ON)
    val list = find("hist_list") ?: return
    list.setGestureMargin(device.displayWidth / GESTURE_MARGIN)
    repeat(FLINGS) { list.fling(Direction.DOWN) }
    repeat(FLINGS) { list.fling(Direction.UP) }
}

/** The branch sheet (Ctrl+B), then closed. */
fun MacrobenchmarkScope.openBranchSheet() {
    device.pressKeyCode(KeyEvent.KEYCODE_B, KeyEvent.META_CTRL_ON)
    find("br_sheet")
    device.pressBack()
}

private const val FLINGS = 3
private const val GESTURE_MARGIN = 5
