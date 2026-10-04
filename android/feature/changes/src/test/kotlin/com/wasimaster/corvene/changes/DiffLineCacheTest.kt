package com.wasimaster.corvene.changes

import androidx.compose.ui.text.font.FontStyle
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.design.diffPaletteOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertSame
import org.junit.Test

/** Highlighted lines are built once per (generation, row) and palette. */
class DiffLineCacheTest {

    private val light = diffPaletteOf(DesignStyle.GitHubMobile, dark = false)
    private val dark = diffPaletteOf(DesignStyle.GitHubMobile, dark = true)

    @Test
    fun `spans become colours, comments italic`() {
        val line = highlight(SampleRows[4], light)
        val styles = line.spanStyles
        assertEquals(light.syntax.keyword, styles[0].item.color)
        assertEquals(light.syntax.string, styles[1].item.color)
        assertEquals(FontStyle.Italic, styles[2].item.fontStyle)
    }

    @Test
    fun `the cache reuses lines and forgets them on a palette change`() {
        val cache = DiffLineCache(capacity = 2)
        val first = cache.line(3, SampleRows[1], light)
        assertSame(first, cache.line(3, SampleRows[1], light))
        assertNotSame(first, cache.line(3, SampleRows[1], dark))
        cache.line(3, SampleRows[2], dark)
        cache.line(3, SampleRows[3], dark)
        assertEquals(2, cache.size)
    }
}
