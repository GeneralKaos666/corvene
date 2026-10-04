package com.wasimaster.corvene.changes

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontStyle
import com.wasimaster.corvene.design.DiffPalette
import com.wasimaster.corvene.ffi.gen.DiffRowKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowVm
import com.wasimaster.corvene.ffi.gen.TokenClassVm

/**
 * Highlighted diff lines, built once: an LRU of [capacity] `AnnotatedString`s
 * keyed by (generation, row index), cleared when the palette changes (a theme
 * or style switch). Main thread only, like the composition reading it.
 */
class DiffLineCache(private val capacity: Int = CAPACITY) {

    private var palette: DiffPalette? = null
    private val lines = object : LinkedHashMap<Long, AnnotatedString>(INITIAL, LOAD_FACTOR, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, AnnotatedString>?) = size > capacity
    }

    val size: Int get() = lines.size

    fun line(generation: Long, row: DiffRowVm, palette: DiffPalette): AnnotatedString {
        if (palette !== this.palette) {
            lines.clear()
            this.palette = palette
        }
        val key = (generation shl INDEX_BITS) or row.index.toLong()
        return lines.getOrPut(key) { highlight(row, palette) }
    }

    private companion object {
        const val CAPACITY = 4096
        const val INITIAL = 256
        const val LOAD_FACTOR = 0.75f
        const val INDEX_BITS = 32
    }
}

/** The row's text with its syntax spans (UTF-16 ranges from the engine). */
fun highlight(row: DiffRowVm, palette: DiffPalette): AnnotatedString {
    if (row.kind == DiffRowKindVm.HUNK || row.spans.isEmpty()) return AnnotatedString(row.text)
    val builder = AnnotatedString.Builder(row.text)
    val length = row.text.length
    for (span in row.spans) {
        val color = palette.colorOf(span.`class`)
        val start = span.start.toInt().coerceIn(0, length)
        val end = span.end.toInt().coerceIn(start, length)
        if (start == end || (color == null && span.`class` != TokenClassVm.COMMENT)) continue
        val italic = if (span.`class` == TokenClassVm.COMMENT) FontStyle.Italic else null
        builder.addStyle(SpanStyle(color = color ?: Color.Unspecified, fontStyle = italic), start, end)
    }
    return builder.toAnnotatedString()
}

/**
 * GHD's CodeMirror classes onto Primer's prettylights roles (the desktop's
 * `syntax_color`): null keeps the row's text colour.
 */
internal fun DiffPalette.colorOf(token: TokenClassVm): Color? = when (token) {
    TokenClassVm.KEYWORD -> syntax.keyword
    TokenClassVm.ATOM -> syntax.atom
    TokenClassVm.VARIABLE -> syntax.variable
    TokenClassVm.ALT_VARIABLE -> syntax.altVariable
    TokenClassVm.TYPE -> syntax.type
    TokenClassVm.COMMENT -> syntax.comment
    TokenClassVm.STRING -> syntax.string
    TokenClassVm.QUALIFIER -> syntax.qualifier
    TokenClassVm.TAG -> syntax.tag
    TokenClassVm.ATTRIBUTE -> syntax.attribute
    TokenClassVm.HEADER -> syntax.header
    TokenClassVm.QUOTE -> syntax.quote
    TokenClassVm.LINK -> syntax.link
}
