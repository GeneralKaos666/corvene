package com.wasimaster.corvene.benchmark

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The baseline profile (design §5): launch → repository list → a
 * repository → Changes → a diff, scrolled → History, scrolled → the branch
 * sheet. Startup is also written as the startup profile (dex layout).
 *
 *     ./gradlew :app:generateFossReleaseBaselineProfile -Pcorvene.benchmark=true -Pcorvene.abis=arm64-v8a
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {

    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun generate() = rule.collect(packageName = TARGET, includeInStartupProfile = true) {
        pressHome()
        startActivityAndWait()
        openFirstRepository()
        scrollFirstDiff()
        scrollHistory()
        openBranchSheet()
    }
}
