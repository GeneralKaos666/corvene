package com.wasimaster.corvene.changes

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.DesignStyleSamples
import com.wasimaster.corvene.ffi.gen.ChangedFileVm
import com.wasimaster.corvene.ffi.gen.ChangesVm
import com.wasimaster.corvene.ffi.gen.CommitFormVm
import com.wasimaster.corvene.ffi.gen.DiffHeaderVm
import com.wasimaster.corvene.ffi.gen.DiffKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowVm
import com.wasimaster.corvene.ffi.gen.FileStatusVm
import com.wasimaster.corvene.ffi.gen.IncludeVm
import com.wasimaster.corvene.ffi.gen.SpanVm
import com.wasimaster.corvene.ffi.gen.TokenClassVm

// View models for previews and tests: a few changed files, a small Kotlin diff.

internal val SampleForm = CommitFormVm(
    author = "wasi-master",
    branch = "main",
    committing = false,
    amending = false,
    coAuthors = emptyList(),
    lastCommitSha = null,
    lastCommitSummary = null,
)

internal val SampleChanges = ChangesVm(
    repo = 1u,
    loading = false,
    error = null,
    files = listOf(
        ChangedFileVm("README.md", null, FileStatusVm.MODIFIED, IncludeVm.ALL, true, 1u, 0u),
        ChangedFileVm(
            "android/feature/changes/src/main/kotlin/ChangesScreen.kt",
            null,
            FileStatusVm.MODIFIED,
            IncludeVm.PARTIAL,
            false,
            42u,
            7u,
        ),
        ChangedFileVm("docs/new-guide.md", null, FileStatusVm.UNTRACKED, IncludeVm.ALL, false, 12u, 0u),
        ChangedFileVm("old/removed.txt", null, FileStatusVm.DELETED, IncludeVm.NONE, false, 0u, 3u),
        ChangedFileVm("src/renamed.rs", "src/original.rs", FileStatusVm.RENAMED, IncludeVm.ALL, false, 0u, 0u),
    ),
    includedCount = 4u,
    selectedFile = "README.md",
    diffGeneration = 3u,
    commitNonce = 0u,
    form = SampleForm,
    conflicts = 0u,
    stashCount = 1u,
    filterIncluded = false,
    filterExcluded = false,
    filterNew = false,
    filterModified = false,
    filterDeleted = false,
    renamedFilterAvailable = true,
    filterRenamed = false,
    stashOnCurrentBranch = true,
)

internal val EmptyChanges = SampleChanges.copy(files = emptyList(), includedCount = 0u, selectedFile = null, stashCount = 0u)

internal val SampleHeader = DiffHeaderVm(
    repo = 1u,
    path = "app/src/main/kotlin/Greeter.kt",
    kind = DiffKindVm.TEXT,
    generation = 3u,
    rowCount = 9u,
    hunkCount = 1u,
    linesAdded = 3u,
    linesDeleted = 1u,
    include = IncludeVm.PARTIAL,
)

private fun row(
    index: Int,
    kind: DiffRowKindVm,
    text: String,
    old: Int?,
    new: Int?,
    selected: Boolean = true,
    spans: List<SpanVm> = emptyList(),
) = DiffRowVm(index.toUInt(), kind, text, old?.toUInt(), new?.toUInt(), selected, false, spans)

/** Spans for the first occurrence of each piece after the previous one. */
private fun spans(text: String, vararg pieces: Pair<String, TokenClassVm>): List<SpanVm> {
    var from = 0
    return pieces.map { (piece, token) ->
        val start = text.indexOf(piece, from)
        from = start + piece.length
        SpanVm(start.toUInt(), from.toUInt(), token)
    }
}

private fun code(index: Int, kind: DiffRowKindVm, text: String, old: Int?, new: Int?, vararg pieces: Pair<String, TokenClassVm>) = row(
    index,
    kind,
    text,
    old,
    new,
    spans = spans(text, *pieces),
)

private const val GREET = "    fun greet() = \"Hello, \$name\""
private const val GREET_LOUD = "    fun greet(loud: Boolean = false): String {"
private const val TEXT = "        val text = \"Hello, \$name\" // hi"
private const val RETURN = "        return if (loud) text.uppercase() else text"
private const val CLASS = "class Greeter(private val name: String) {"
private const val HELLO = "\"Hello, \$name\""

private val KEYWORD = TokenClassVm.KEYWORD
private val DEF = TokenClassVm.VARIABLE
private val TYPE = TokenClassVm.TYPE
private val STRING = TokenClassVm.STRING
private val ATOM = TokenClassVm.ATOM
private val COMMENT = TokenClassVm.COMMENT

internal val SampleRows = listOf(
    row(0, DiffRowKindVm.HUNK, "@@ -1,6 +1,8 @@ class Greeter", null, null),
    code(1, DiffRowKindVm.CONTEXT, CLASS, 1, 1, "class" to KEYWORD, "Greeter" to DEF, "String" to TYPE),
    code(2, DiffRowKindVm.DELETE, GREET, 2, null, "fun" to KEYWORD, "greet" to DEF, HELLO to STRING),
    code(3, DiffRowKindVm.ADD, GREET_LOUD, null, 2, "fun" to KEYWORD, "greet" to DEF, "false" to ATOM, "String" to TYPE),
    row(4, DiffRowKindVm.ADD, TEXT, null, 3, selected = false, spans = spans(TEXT, "val" to KEYWORD, HELLO to STRING, "// hi" to COMMENT)),
    code(5, DiffRowKindVm.ADD, RETURN, null, 4, "return" to KEYWORD, "if" to KEYWORD, "else" to KEYWORD),
    row(6, DiffRowKindVm.ADD, "    }", null, 5),
    row(7, DiffRowKindVm.CONTEXT, "}", 3, 6),
    row(8, DiffRowKindVm.CONTEXT, "", 4, 7),
)

/** A pager over [SampleRows] (previews and tests). */
internal fun samplePager(rows: List<DiffRowVm> = SampleRows, generation: Long = 3): DiffPager =
    DiffPager(generation, rows.size) { start, count -> rows.drop(start).take(count) }

internal object NoChangesActions : ChangesActions {
    override fun select(path: String) = Unit
    override fun toggleIncluded(path: String) = Unit
    override fun discard(paths: List<String>) = Unit
    override fun toggleFilter(option: FilterOption) = Unit
    override fun clearFilters() = Unit
    override fun commit(summary: String, description: String) = Unit
    override fun undo() = Unit
    override fun restoreStash() = Unit
}

internal object NoDiffActions : DiffActions {
    override fun toggleFileIncluded() = Unit
    override fun toggleLine(index: Int) = Unit
    override fun setHideWhitespace(hide: Boolean) = Unit
}

@Preview(widthDp = 360, heightDp = 1800)
@Composable
private fun ChangesScreenPreview() {
    DesignStyleSamples {
        Box(Modifier.size(360.dp, 560.dp)) {
            ChangesScreen(SampleChanges, confirmDiscard = true, actions = NoChangesActions)
        }
    }
}

@Preview(widthDp = 360, heightDp = 1800)
@Composable
private fun NoChangesPreview() {
    DesignStyleSamples {
        Box(Modifier.size(360.dp, 560.dp)) {
            ChangesScreen(EmptyChanges, confirmDiscard = true, actions = NoChangesActions)
        }
    }
}

@Preview(widthDp = 360, heightDp = 1200)
@Composable
private fun DiffScreenPreview() {
    DesignStyleSamples {
        Box(Modifier.size(360.dp, 360.dp)) {
            DiffScreen(SampleHeader, remember { samplePager() }, remember { DiffLineCache() }, false, NoDiffActions)
        }
    }
}
