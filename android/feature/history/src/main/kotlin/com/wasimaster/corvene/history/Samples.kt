package com.wasimaster.corvene.history

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.tooling.preview.Preview
import com.wasimaster.corvene.design.DesignStyleSamples
import com.wasimaster.corvene.ffi.gen.CommitDetailVm
import com.wasimaster.corvene.ffi.gen.CommitFileVm
import com.wasimaster.corvene.ffi.gen.CommitVm
import com.wasimaster.corvene.ffi.gen.DiffKindVm
import com.wasimaster.corvene.ffi.gen.FileStatusVm
import com.wasimaster.corvene.ffi.gen.HistoryVm

// View models for previews and tests: a short history, one commit's detail.

/** A fixed "now" (2026-10-04) so relative times never change in screenshots. */
internal const val SAMPLE_NOW = 1_791_100_000_000L
private const val SECONDS = 1_791_100_000L
private const val HOUR = 3600L

internal val SampleCommits = listOf(
    CommitVm("a1b2c3d4e5", "Merge branch 'feature-a'", "Wasi Master", "w@example.com", SECONDS - HOUR, emptyList(), false, isMerge = true),
    CommitVm("b2c3d4e5f6", "Add the branch sheet", "Wasi Master", "w@example.com", SECONDS - 3 * HOUR, listOf("v0.2.0"), true, false),
    CommitVm(
        "c3d4e5f6a7",
        "Page the history in windows of a hundred",
        "Octo Cat",
        "o@example.com",
        SECONDS - 26 * HOUR,
        emptyList(),
        false,
        false,
    ),
    CommitVm("d4e5f6a7b8", "Add main.rs", "Wasi Master", "w@example.com", SECONDS - 80 * HOUR, emptyList(), false, false),
    CommitVm("e5f6a7b8c9", "Initial commit", "Wasi Master", "w@example.com", SECONDS - 200 * HOUR, listOf("v0.1.0", "first"), false, false),
)

internal val SampleHistory = HistoryVm(
    repo = 1u,
    loading = false,
    exhausted = true,
    totalLoaded = SampleCommits.size.toUInt(),
    selected = listOf("b2c3d4e5f6"),
    start = 0u,
    commits = emptyList(),
    compare = null,
)

internal fun samplePager(commits: List<CommitVm> = SampleCommits) = CommitPager(commits.size) { start, count ->
    commits.drop(start).take(count)
}

internal val SampleDetail = CommitDetailVm(
    repo = 1u,
    shas = listOf("b2c3d4e5f6"),
    summary = "Add the branch sheet",
    body = "The branch chip opens a SelectPanel with the groups Default, Recent and Other.\n\n" +
        "Long press opens the branch menu.\nRemote branches fold under Remote.\nThe pull requests tab lists GitHub's.",
    authorName = "Wasi Master",
    authorEmail = "w@example.com",
    authoredAt = SECONDS - 3 * HOUR,
    files = listOf(
        CommitFileVm("android/feature/branches/BranchSheet.kt", null, FileStatusVm.NEW, true),
        CommitFileVm("android/app/CorveneScaffold.kt", null, FileStatusVm.MODIFIED, false),
        CommitFileVm("old/Picker.kt", null, FileStatusVm.DELETED, false),
        CommitFileVm("src/renamed.rs", "src/original.rs", FileStatusVm.RENAMED, false),
    ),
    linesAdded = 412u,
    linesDeleted = 37u,
    selectedFile = "android/feature/branches/BranchSheet.kt",
    diffKind = DiffKindVm.TEXT,
    diffGeneration = 1u,
    diffRowCount = 10u,
    diffLinesAdded = 9u,
    diffLinesDeleted = 0u,
)

internal object NoHistoryActions : HistoryActions {
    override fun select(sha: String) = Unit

    override fun selectRange(shas: List<String>) = Unit

    override fun loadMore() = Unit

    override fun compare(branch: String?, mode: String) = Unit

    override fun merge(branch: String) = Unit

    override fun revert(sha: String) = Unit

    override fun cherryPick(shas: List<String>) = Unit

    override fun createBranchFrom(sha: String) = Unit

    override fun createTag(sha: String, name: String, message: String) = Unit

    override fun checkout(sha: String) = Unit

    override fun reset(sha: String) = Unit

    override fun copySha(sha: String) = Unit

    override fun viewOnGitHub(sha: String) = Unit
}

internal object NoDetailActions : CommitDetailActions {
    override fun selectFile(path: String) = Unit

    override fun copySha(sha: String) = Unit
}

@Preview(widthDp = 360, heightDp = 1600)
@Composable
private fun HistoryScreenPreview() {
    DesignStyleSamples {
        HistoryScreen(
            SampleHistory,
            remember { samplePager() },
            null,
            listOf("main", "feature-a"),
            "main",
            true,
            NoHistoryActions,
            now = SAMPLE_NOW,
        )
    }
}

@Preview(widthDp = 360, heightDp = 1600)
@Composable
private fun CommitDetailPreview() {
    DesignStyleSamples { CommitDetailScreen(SampleDetail, NoDetailActions, now = SAMPLE_NOW) }
}
