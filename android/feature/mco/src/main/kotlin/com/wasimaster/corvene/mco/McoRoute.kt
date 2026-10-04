package com.wasimaster.corvene.mco

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.McoKindVm
import com.wasimaster.corvene.ffi.gen.McoStepVm
import com.wasimaster.corvene.ffi.gen.ResolutionVm
import com.wasimaster.corvene.ffi.rememberCoreQuery

/**
 * GHD's `MultiCommitOperation` popup for [repo], by the engine's step
 * (`mco(repo)`): ChooseBranch → [ChooseBranchSheet] (rebase: the base;
 * cherry-pick: the target, or a new branch), WarnForcePush →
 * [WarnForcePushDialog], ShowProgress → [McoProgressDialog], ShowConflicts →
 * [ConflictsScreen] full screen, ConfirmAbort → [ConfirmAbortDialog],
 * CreateBranch → [NewBranchNameDialog]. Closing a step closes the popup
 * (`closePopup`); closing the conflicts hides them (`hideConflicts`, the
 * banner offers them again).
 */
@Composable
fun MultiCommitOperationRoute(repo: Long, modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val id = repo.toULong()
    val mco by rememberCoreQuery(repo) { mco(id) }
    val conflicts by rememberCoreQuery(repo) { conflicts(id) }
    val branches by rememberCoreQuery(repo) { branches(id) }
    val close = { core.dispatch { closePopup() } }
    val op = mco.value ?: return
    val names = branches.value?.branches?.map { it.name }.orEmpty()
    when (val step = op.step) {
        McoStepVm.ChooseBranch -> {
            var naming by rememberSaveable { mutableStateOf(false) }
            val cherryPick = op.kind == McoKindVm.CHERRY_PICK
            ChooseBranchSheet(
                title = if (cherryPick) {
                    stringResource(R.string.mco_choose_cherry_pick, op.commitCount.toInt())
                } else {
                    stringResource(R.string.mco_choose_rebase, op.targetBranch.orEmpty())
                },
                branches = names,
                current = branches.value?.current,
                onChoose = { name ->
                    if (cherryPick) core.dispatch { cherryPickToBranch(id, name) } else core.dispatch { startRebase(id, name) }
                },
                onDismissRequest = close,
                modifier = modifier,
                onNewBranch = if (cherryPick) ({ naming = true }) else null,
            )
            if (naming) {
                NewBranchNameDialog(
                    "",
                    onCreate = { name ->
                        naming = false
                        core.dispatch { cherryPickToNewBranch(id, name, null) }
                    },
                    onDismissRequest = { naming = false },
                )
            }
        }
        is McoStepVm.CreateBranch -> NewBranchNameDialog(
            step.initialName,
            onCreate = { name -> core.dispatch { cherryPickToNewBranch(id, name, null) } },
            onDismissRequest = close,
        )
        McoStepVm.WarnForcePush -> WarnForcePushDialog(op.kind, onBegin = null, onDismissRequest = close, modifier = modifier)
        McoStepVm.ShowProgress -> McoProgressDialog(op, modifier)
        McoStepVm.ShowConflicts -> {
            val value = conflicts.value ?: return
            val actions = remember(core, id, op.userHasResolvedConflicts, value) {
                object : ConflictsActions {
                    override fun resolve(
                        path: String,
                        resolution: ResolutionVm?,
                    ) = core.dispatch { setManualResolution(id, path, resolution) }

                    override fun abort() {
                        // GHD `request_abort_mco`: confirm when resolutions would be lost
                        if (op.userHasResolvedConflicts || value.files.any { it.resolution != null }) {
                            core.dispatch { setMcoStep(id, "confirm-abort") }
                        } else {
                            core.dispatch { abortMco(id) }
                        }
                    }

                    override fun continueOperation() = core.dispatch { continueAfterConflicts(id) }

                    override fun close() = core.dispatch { hideConflicts(id) }
                }
            }
            Dialog(
                onDismissRequest = actions::close,
                properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
            ) {
                ConflictsScreen(value, actions, modifier)
            }
        }
        McoStepVm.ConfirmAbort -> ConfirmAbortDialog(
            op.kind,
            onAbort = { core.dispatch { abortMco(id) } },
            onDismissRequest = { core.dispatch { setMcoStep(id, "show-conflicts") } },
            modifier = modifier,
        )
        McoStepVm.HideConflicts -> Unit
    }
}
