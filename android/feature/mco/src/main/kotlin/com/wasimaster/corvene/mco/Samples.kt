package com.wasimaster.corvene.mco

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.wasimaster.corvene.design.DesignStyleSamples
import com.wasimaster.corvene.ffi.gen.ConflictedFileVm
import com.wasimaster.corvene.ffi.gen.ConflictsVm
import com.wasimaster.corvene.ffi.gen.McoKindVm
import com.wasimaster.corvene.ffi.gen.ResolutionVm

// View models for previews and tests: a rebase with two conflicted files, one resolved.

internal val SampleConflicts = ConflictsVm(
    repo = 1u,
    kind = McoKindVm.REBASE,
    currentBranch = null,
    targetBranch = "feature-a",
    files = listOf(
        ConflictedFileVm("src/main.rs", resolution = null, unresolved = true),
        ConflictedFileVm("README.md", resolution = ResolutionVm.OURS, unresolved = false),
    ),
    unresolvedCount = 1u,
)

internal object NoConflictsActions : ConflictsActions {
    override fun resolve(path: String, resolution: ResolutionVm?) = Unit

    override fun abort() = Unit

    override fun continueOperation() = Unit

    override fun close() = Unit
}

@Preview(widthDp = 360, heightDp = 1800)
@Composable
private fun ConflictsScreenPreview() {
    DesignStyleSamples { ConflictsScreen(SampleConflicts, NoConflictsActions) }
}
