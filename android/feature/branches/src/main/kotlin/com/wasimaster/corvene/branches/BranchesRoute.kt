package com.wasimaster.corvene.branches

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.wasimaster.corvene.design.SyncButtonModel
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.rememberCoreQuery

/**
 * The branch panel wired to the engine: `branches(repo)`, the pull requests
 * when [github] (the repository is on GitHub), every action a dispatch.
 * [onCompare] moves to History once the comparison is asked for.
 */
@Composable
fun BranchSheetRoute(
    repo: Long,
    github: Boolean,
    onDismissRequest: () -> Unit,
    onCompare: () -> Unit,
    modifier: Modifier = Modifier,
    inline: Boolean = false,
) {
    val core = LocalCore.current
    val id = repo.toULong()
    val branches by rememberCoreQuery(repo) { branches(id) }
    val pulls by rememberCoreQuery(repo, github) { if (github) pullRequests(id) else null }
    val compare by rememberUpdatedState(onCompare)
    val actions = remember(core, id) {
        object : BranchActions {
            override fun checkout(name: String) = core.dispatch { checkoutBranch(id, name, null) }

            override fun create(name: String, startPoint: String?) = core.dispatch { createBranch(id, name, startPoint) }

            override fun rename(old: String, new: String) = core.dispatch { renameBranch(id, old, new) }

            override fun delete(name: String, includeRemote: Boolean) = core.dispatch { deleteBranch(id, name, includeRemote) }

            override fun merge(name: String) = core.dispatch { mergeBranch(id, name, false) }

            override fun rebaseOnto(name: String) = core.dispatch { startRebase(id, name) }

            override fun compare(name: String) {
                core.dispatch {
                    selectSection(id, true)
                    compareToBranch(id, name, "behind")
                }
                compare()
            }

            override fun checkoutPullRequest(number: ULong) = core.dispatch { checkoutPullRequest(id, number) }
        }
    }
    val value = branches.value ?: return
    BranchSheet(value, pulls.value, actions, onDismissRequest, modifier, inline = inline)
}

/**
 * The push/pull button's state for one repository: its [model], what a tap
 * and a long press do, and the menu and dialogs it opens. [Menu] draws in the
 * button's anchor; [Dialogs] anywhere in the repository screen.
 */
@Stable
class SyncController internal constructor(
    private val core: Core,
    private val repo: ULong,
) : SyncActions {
    // plain fields: set on every composition of the owner, read by the menu and dialogs when they open
    internal var branches: BranchesVm? = null
    internal var repositoryName: String = ""
    internal var confirmForcePush: Boolean = true
    private var menuOpen by mutableStateOf(false)
    private var forcing by mutableStateOf(false)
    private var publishing by mutableStateOf(false)

    fun click() = primary(branches?.sync)

    fun longClick() {
        menuOpen = true
    }

    override fun fetch() = core.dispatch { fetch(repo) }

    override fun pull() = core.dispatch { pull(repo) }

    override fun push() = core.dispatch { push(repo, false) }

    override fun forcePush() {
        if (confirmForcePush) forcing = true else core.dispatch { push(repo, true) }
    }

    override fun publishRepository() {
        publishing = true
    }

    @Composable
    fun Menu() {
        SyncMenu(menuOpen, branches, this, onDismissRequest = { menuOpen = false })
    }

    @Composable
    fun Dialogs() {
        if (forcing) {
            val upstream = branches?.branches?.firstOrNull { it.current }?.upstream ?: branches?.current.orEmpty()
            ConfirmForcePushDialog(
                upstream,
                onForcePush = {
                    forcing = false
                    core.dispatch { push(repo, true) }
                },
                onDismissRequest = { forcing = false },
            )
        }
        if (publishing) {
            val session by rememberCoreQuery { session() }
            PublishRepositoryDialog(
                initialName = repositoryName,
                accounts = session.value?.accounts.orEmpty(),
                onPublish = { name, description, private, endpoint, org ->
                    publishing = false
                    core.dispatch { publishRepository(repo, name, description, private, endpoint, org) }
                },
                onDismissRequest = { publishing = false },
            )
        }
    }
}

/** The [SyncController] of [repo], fed the latest [branches] and settings. */
@Composable
fun rememberSyncController(repo: Long, branches: BranchesVm?, repositoryName: String): SyncController {
    val core = LocalCore.current
    val settings by rememberCoreQuery { settings() }
    val controller = remember(core, repo) { SyncController(core, repo.toULong()) }
    controller.branches = branches
    controller.repositoryName = repositoryName
    controller.confirmForcePush = settings.value?.confirmForcePush ?: true
    return controller
}

/** [syncButtonModel] of the controller's latest state. */
@Composable
fun SyncController.model(): SyncButtonModel = syncButtonModel(branches)
