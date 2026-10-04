package com.wasimaster.corvene.branches

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.wasimaster.corvene.design.DesignStyleSamples
import com.wasimaster.corvene.ffi.gen.BranchVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.PullRequestVm
import com.wasimaster.corvene.ffi.gen.PullRequestsVm
import com.wasimaster.corvene.ffi.gen.SyncActionVm

// View models for previews and tests: a few branches, two pull requests.

/** A fixed "now" (2026-10-04) so relative times never change in screenshots. */
internal const val SAMPLE_NOW = 1_791_100_000_000L
private const val SECONDS = 1_791_100_000L
private const val HOUR = 3600L
private const val DAY = 24 * HOUR

internal val SampleBranches = BranchesVm(
    repo = 1u,
    current = "main",
    detachedSha = null,
    defaultBranch = "main",
    recent = listOf("main", "feature-a"),
    branches = listOf(
        BranchVm("main", remote = false, current = true, upstream = "origin/main", tipTime = SECONDS - 2 * HOUR),
        BranchVm("feature-a", remote = false, current = false, upstream = null, tipTime = SECONDS - DAY),
        BranchVm("feature-b", remote = false, current = false, upstream = null, tipTime = SECONDS - 3 * DAY),
        BranchVm("wasi/very-long-branch-name-for-the-chip", remote = false, current = false, upstream = null, tipTime = SECONDS - 40 * DAY),
        BranchVm("origin/main", remote = true, current = false, upstream = null, tipTime = SECONDS - 2 * HOUR),
        BranchVm("origin/release", remote = true, current = false, upstream = null, tipTime = SECONDS - 9 * DAY),
    ),
    ahead = 1u,
    behind = 0u,
    sync = SyncActionVm.PUSH,
    syncProgressTitle = null,
    syncProgress = null,
    lastFetchedAt = SECONDS - 5 * 60,
    stashCount = 0u,
)

internal val SamplePulls = PullRequestsVm(
    repo = 1u,
    pullRequests = listOf(
        PullRequestVm(12u, "Add the branch sheet", "wasi-master", false, "feature-a", "abc", "main", null, "", "", checkedOut = false),
        PullRequestVm(9u, "Draft: history paging", "octocat", true, "history", "def", "main", "octocat/corvene", "", "", false),
    ),
)

internal object NoBranchActions : BranchActions {
    override fun checkout(name: String) = Unit

    override fun create(name: String, startPoint: String?) = Unit

    override fun rename(old: String, new: String) = Unit

    override fun delete(name: String, includeRemote: Boolean) = Unit

    override fun merge(name: String) = Unit

    override fun rebaseOnto(name: String) = Unit

    override fun compare(name: String) = Unit

    override fun checkoutPullRequest(number: ULong) = Unit
}

@Preview(widthDp = 360, heightDp = 1600)
@Composable
private fun BranchSheetPreview() {
    DesignStyleSamples {
        BranchSheet(SampleBranches, SamplePulls, NoBranchActions, {}, inline = true, now = SAMPLE_NOW)
    }
}
