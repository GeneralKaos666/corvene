package com.wasimaster.corvene.benchmark

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Flinging the first changed file's diff (design §5: < 1 % jank on a 5k-line diff). */
@RunWith(AndroidJUnit4::class)
class DiffScrollBenchmark {

    @get:Rule
    val rule = MacrobenchmarkRule()

    @Test
    fun diffScroll() = rule.measureRepeated(
        packageName = TARGET,
        metrics = listOf(FrameTimingMetric()),
        compilationMode = CompilationMode.Partial(BaselineProfileMode.Require),
        startupMode = StartupMode.WARM,
        iterations = ITERATIONS,
        setupBlock = {
            pressHome()
            startActivityAndWait()
            openFirstRepository()
        },
    ) {
        scrollFirstDiff()
    }
}

/** Flinging the History list. */
@RunWith(AndroidJUnit4::class)
class HistoryScrollBenchmark {

    @get:Rule
    val rule = MacrobenchmarkRule()

    @Test
    fun historyScroll() = rule.measureRepeated(
        packageName = TARGET,
        metrics = listOf(FrameTimingMetric()),
        compilationMode = CompilationMode.Partial(BaselineProfileMode.Require),
        startupMode = StartupMode.WARM,
        iterations = ITERATIONS,
        setupBlock = {
            pressHome()
            startActivityAndWait()
            openFirstRepository()
        },
    ) {
        scrollHistory()
    }
}

private const val ITERATIONS = 5
