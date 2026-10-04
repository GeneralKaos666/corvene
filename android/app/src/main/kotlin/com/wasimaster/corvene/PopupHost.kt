package com.wasimaster.corvene

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.wasimaster.corvene.branches.ConfirmForcePushDialog
import com.wasimaster.corvene.branches.ConfirmOverwriteStashDialog
import com.wasimaster.corvene.branches.ConfirmSwitchBranchDialog
import com.wasimaster.corvene.branches.CreateBranchDialog
import com.wasimaster.corvene.branches.DeleteBranchDialog
import com.wasimaster.corvene.branches.GenericGitAuthenticationDialog
import com.wasimaster.corvene.branches.PublishRepositoryDialog
import com.wasimaster.corvene.branches.RenameBranchDialog
import com.wasimaster.corvene.branches.StashAndSwitchBranchDialog
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.Corvene
import com.wasimaster.corvene.ffi.gen.PopupVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.history.ConfirmCheckoutCommitDialog
import com.wasimaster.corvene.history.CreateTagDialog
import com.wasimaster.corvene.history.WarningBeforeResetDialog
import com.wasimaster.corvene.mco.ChooseBranchSheet
import com.wasimaster.corvene.mco.MultiCommitOperationRoute
import com.wasimaster.corvene.repositories.AddRepositoryRoute
import com.wasimaster.corvene.repositories.CloneRoute
import com.wasimaster.corvene.repositories.CreateRepositoryRoute

/**
 * The engine's open dialog (`popup()`, GHD's popup stack) wired to the
 * engine: the dialog's repository's branches and the accounts for the forms,
 * [CorePopupActions] for what the buttons do. The drawing is [PopupDialog].
 */
@Composable
fun PopupHost(onSignIn: (enterprise: Boolean) -> Unit) {
    val core = LocalCore.current
    val popup by rememberCoreQuery { popup() ?: NoPopup }
    val value = popup.value?.takeIf { it.kind.isNotEmpty() } ?: return
    val close: () -> Unit = { core.dispatch { closePopup() } }
    // GHD's full-screen dialogs: the same screens the app's own menus open
    when (value.kind) {
        "CloneRepository", "CloneRepositoryRetry" -> return FullScreenPopup(close) {
            CloneRoute(
                value.field("url"),
                onClose = close,
                onSignIn = {
                    close()
                    onSignIn(false)
                },
            )
        }
        "AddExistingRepository" -> return FullScreenPopup(close) { AddRepositoryRoute(onClose = close, initialPath = value.field("path")) }
        "CreateRepository" -> return FullScreenPopup(close) { CreateRepositoryRoute(onClose = close, initialBase = value.field("path")) }
        "SignIn" -> {
            // the sign-in screen is a destination; the popup only asks for it
            LaunchedEffect(value) {
                close()
                onSignIn(value.field("enterprise") == "true")
            }
            return
        }
    }
    val repo = value.repo
    val branches by rememberCoreQuery(repo) { repo?.let { branches(it) } }
    val session by rememberCoreQuery(value.kind) { if (value.kind == "PublishRepository") session() else null }
    val list by rememberCoreQuery(value.kind) { if (value.kind == "PublishRepository") repoList() else null }
    val actions = remember(core) { CorePopupActions(core) }
    PopupDialog(
        value,
        PopupContext(
            branches = branches.value,
            accounts = session.value?.accounts.orEmpty(),
            repositoryName = list.value?.repositories?.firstOrNull { it.id == repo }?.name.orEmpty(),
        ),
        actions,
        multiCommitOperation = { id -> MultiCommitOperationRoute(id.toLong()) },
    )
}

/** A dialog window over everything, the whole screen, edge to edge. */
@Composable
private fun FullScreenPopup(onDismissRequest: () -> Unit, content: @Composable () -> Unit) {
    Dialog(
        onDismissRequest = onDismissRequest,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
        content = content,
    )
}

/** What a dialog needs besides its own fields. */
@Immutable
data class PopupContext(
    val branches: BranchesVm? = null,
    val accounts: List<AccountVm> = emptyList(),
    val repositoryName: String = "",
)

/** What the dialogs' buttons do: each one an engine action (after `closePopup`, as GHD's dialogs do). */
interface PopupActions {
    fun close()

    fun discardChanges(repo: ULong, paths: List<String>)

    fun deleteBranch(repo: ULong, name: String, includeRemote: Boolean)

    fun renameBranch(repo: ULong, old: String, new: String)

    fun createBranch(repo: ULong, name: String, startPoint: String?)

    fun createTag(repo: ULong, name: String, sha: String, message: String)

    fun push(repo: ULong, force: Boolean)

    fun pull(repo: ULong)

    fun fetch(repo: ULong)

    fun checkoutBranch(repo: ULong, name: String, strategy: String?)

    fun dropStash(repo: ULong)

    fun stashAllChanges(repo: ULong)

    fun checkoutCommit(repo: ULong, sha: String)

    fun resetToCommit(repo: ULong, sha: String)

    fun undoCommit(repo: ULong)

    fun commit(repo: ULong, summary: String, description: String)

    fun removeRepository(repo: ULong)

    fun mergeBranch(repo: ULong, branch: String, squash: Boolean)

    fun submitGenericAuth(username: String, password: String)

    fun retry()

    fun publishRepository(repo: ULong, name: String, description: String, private: Boolean, endpoint: String, org: String?)
}

/** [PopupActions] as engine dispatches, in order on the engine's thread. */
class CorePopupActions(private val core: Core) : PopupActions {
    override fun close() = core.dispatch { closePopup() }

    private fun closeThen(block: Corvene.() -> Unit) = core.dispatch {
        closePopup()
        block()
    }

    override fun discardChanges(repo: ULong, paths: List<String>) = closeThen { discardChanges(repo, paths) }

    override fun deleteBranch(repo: ULong, name: String, includeRemote: Boolean) = closeThen { deleteBranch(repo, name, includeRemote) }

    override fun renameBranch(repo: ULong, old: String, new: String) = closeThen { renameBranch(repo, old, new) }

    override fun createBranch(repo: ULong, name: String, startPoint: String?) = closeThen { createBranch(repo, name, startPoint) }

    override fun createTag(repo: ULong, name: String, sha: String, message: String) = closeThen { createTag(repo, name, sha, message) }

    override fun push(repo: ULong, force: Boolean) = closeThen { push(repo, force) }

    override fun pull(repo: ULong) = closeThen { pull(repo) }

    override fun fetch(repo: ULong) = closeThen { fetch(repo) }

    override fun checkoutBranch(repo: ULong, name: String, strategy: String?) = closeThen { checkoutBranch(repo, name, strategy) }

    override fun dropStash(repo: ULong) = closeThen { dropStash(repo) }

    // the dialog stays: Retry follows once the stash is made
    override fun stashAllChanges(repo: ULong) = core.dispatch { stashAllChanges(repo) }

    override fun checkoutCommit(repo: ULong, sha: String) = closeThen { checkoutCommit(repo, sha) }

    override fun resetToCommit(repo: ULong, sha: String) = closeThen { resetToCommit(repo, sha) }

    override fun undoCommit(repo: ULong) = closeThen { undoCommit(repo) }

    override fun commit(repo: ULong, summary: String, description: String) = closeThen { commit(repo, summary, description) }

    override fun removeRepository(repo: ULong) = closeThen { removeRepository(repo) }

    override fun mergeBranch(repo: ULong, branch: String, squash: Boolean) = closeThen { mergeBranch(repo, branch, squash) }

    // both close the dialog themselves before retrying
    override fun submitGenericAuth(username: String, password: String) = core.dispatch { submitGenericAuth(username, password) }

    override fun retry() = core.dispatch { retryPopupAction() }

    override fun publishRepository(repo: ULong, name: String, description: String, private: Boolean, endpoint: String, org: String?) =
        closeThen { publishRepository(repo, name, description, private, endpoint, org) }
}

/**
 * One `when` over the popup kinds the engine produces (Corvene's `Popup`
 * variant names; GHD's `PopupType` names where they differ are accepted
 * too): each kind its dialog with its primary action, read from
 * [PopupVm.fields] / [PopupVm.lists]. [multiCommitOperation] draws the
 * wizard (it queries the engine itself). Anything else: [GenericPopup].
 */
@Composable
fun PopupDialog(
    popup: PopupVm,
    context: PopupContext,
    actions: PopupActions,
    multiCommitOperation: @Composable (ULong) -> Unit = {},
) {
    val close = actions::close
    val repo = popup.repo
    val branch = popup.field("branch").orEmpty()
    val names = context.branches?.branches?.map { it.name }?.toSet().orEmpty()
    when (popup.kind) {
        "Error" -> Error(popup, close)
        "DiscardChanges", "ConfirmDiscardChanges" -> ConfirmDiscardChanges(popup, actions)
        "DeleteBranch", "DeleteRemoteBranch" -> WithRepo(repo, popup, close) { id ->
            val name = popup.field("name").orEmpty()
            DeleteBranchDialog(
                name,
                hasRemote = popup.kind == "DeleteRemoteBranch" ||
                    context.branches?.branches?.firstOrNull { it.name == name }?.upstream != null,
                onDelete = { remote -> actions.deleteBranch(id, name, remote) },
                onDismissRequest = close,
            )
        }
        "RenameBranch" -> WithRepo(repo, popup, close) { id ->
            val name = popup.field("name").orEmpty()
            RenameBranchDialog(name, names, onRename = { actions.renameBranch(id, name, it) }, onDismissRequest = close)
        }
        "CreateBranch" -> WithRepo(repo, popup, close) { id ->
            CreateBranchDialog(
                initialName = popup.field("initial_name").orEmpty(),
                current = context.branches?.current,
                defaultBranch = context.branches?.defaultBranch,
                targetSha = popup.field("target_sha"),
                existing = names,
                onCreate = { name, start -> actions.createBranch(id, name, start) },
                onDismissRequest = close,
            )
        }
        "CreateTag" -> WithRepo(repo, popup, close) { id ->
            val sha = popup.field("sha").orEmpty()
            CreateTagDialog(sha, onCreate = { name, message -> actions.createTag(id, name, sha, message) }, onDismissRequest = close)
        }
        "ConfirmForcePush" -> WithRepo(repo, popup, close) { id ->
            ConfirmForcePushDialog(
                popup.field("upstream_branch").orEmpty(),
                onForcePush = { actions.push(id, true) },
                onDismissRequest = close,
            )
        }
        "PushNeedsPull" -> WithRepo(repo, popup, close) { id ->
            PushNeedsPull(onPull = { actions.pull(id) }, onFetch = { actions.fetch(id) }, close)
        }
        "UpstreamAlreadyExists" -> Notice(
            stringResource(R.string.app_upstream_title),
            stringResource(R.string.app_upstream_body, popup.field("existing_url").orEmpty()),
            close,
        )
        "StashAndSwitchBranch" -> WithRepo(repo, popup, close) { id ->
            StashAndSwitchBranchDialog(
                current = context.branches?.current,
                target = branch,
                hasStash = (context.branches?.stashCount ?: 0u) > 0u,
                onSwitch = { strategy -> actions.checkoutBranch(id, branch, strategy) },
                onDismissRequest = close,
            )
        }
        "ConfirmOverwriteStash" -> WithRepo(repo, popup, close) { id ->
            ConfirmOverwriteStashDialog(onOverwrite = { actions.checkoutBranch(id, branch, "stash") }, onDismissRequest = close)
        }
        "ConfirmSwitchBranch" -> WithRepo(repo, popup, close) { id ->
            ConfirmSwitchBranchDialog(branch, onSwitch = { actions.checkoutBranch(id, branch, null) }, onDismissRequest = close)
        }
        "ConfirmDiscardStash" -> WithRepo(repo, popup, close) { id ->
            Confirm(
                stringResource(R.string.app_discard_stash_title),
                stringResource(R.string.app_discard_stash_body),
                stringResource(R.string.app_discard_stash_confirm),
                danger = true,
                onConfirm = { actions.dropStash(id) },
                onDismiss = close,
            )
        }
        "CheckoutCommit", "ConfirmCheckoutCommit" -> WithRepo(repo, popup, close) { id ->
            ConfirmCheckoutCommitDialog(onCheckout = { actions.checkoutCommit(id, popup.field("sha").orEmpty()) }, onDismissRequest = close)
        }
        "ResetToCommit", "WarningBeforeReset" -> WithRepo(repo, popup, close) { id ->
            WarningBeforeResetDialog(onReset = { actions.resetToCommit(id, popup.field("sha").orEmpty()) }, onDismissRequest = close)
        }
        "WarnLocalChangesBeforeUndo" -> WithRepo(repo, popup, close) { id ->
            Confirm(
                stringResource(R.string.app_undo_title),
                stringResource(R.string.app_undo_body),
                stringResource(R.string.app_undo_confirm),
                danger = false,
                onConfirm = { actions.undoCommit(id) },
                onDismiss = close,
            )
        }
        "UnknownAuthors" -> WithRepo(repo, popup, close) { id ->
            Confirm(
                stringResource(R.string.app_unknown_authors_title),
                stringResource(R.string.app_unknown_authors_body),
                stringResource(R.string.app_unknown_authors_confirm),
                danger = false,
                onConfirm = { actions.commit(id, popup.field("summary").orEmpty(), popup.field("description").orEmpty()) },
                onDismiss = close,
                items = popup.list("usernames").map { "@$it" },
            )
        }
        "LocalChangesOverwritten" -> WithRepo(repo, popup, close) { id ->
            LocalChangesOverwritten(popup, { actions.stashAllChanges(id) }, actions::retry, close)
        }
        "ConfirmRemoveRepository" -> WithRepo(repo, popup, close) { id ->
            Confirm(
                stringResource(R.string.app_remove_title),
                stringResource(R.string.app_remove_body),
                stringResource(R.string.app_remove_confirm),
                danger = true,
                onConfirm = { actions.removeRepository(id) },
                onDismiss = close,
            )
        }
        "ExternalEditorError", "ExternalEditorFailed" ->
            Notice(stringResource(R.string.app_editor_error), popup.field("message").orEmpty(), close)
        "ShellError", "OpenShellFailed" -> Notice(stringResource(R.string.app_shell_error), popup.field("message").orEmpty(), close)
        "CommitConflictsWarning" -> Notice(
            stringResource(R.string.app_commit_conflicts_title),
            stringResource(R.string.app_commit_conflicts_body),
            close,
        )
        "HookFailed" -> Notice(
            stringResource(R.string.app_hook_failed_title),
            popup.field("message") ?: popup.field("output").orEmpty(),
            close,
        )
        "MergeBranch" -> WithRepo(repo, popup, close) { id ->
            val squash = popup.field("squash") == "true"
            val current = context.branches?.current.orEmpty()
            ChooseBranchSheet(
                title = stringResource(if (squash) R.string.app_squash_merge_into else R.string.app_merge_into, current),
                branches = context.branches?.branches?.map { it.name }.orEmpty(),
                current = context.branches?.current,
                onChoose = { name -> actions.mergeBranch(id, name, squash) },
                onDismissRequest = close,
            )
        }
        "MultiCommitOperation" -> WithRepo(repo, popup, close) { id -> multiCommitOperation(id) }
        "CICheckRunRerun" -> Notice(
            stringResource(R.string.app_rerun_title),
            stringResource(R.string.app_rerun_body, popup.field("git_ref").orEmpty()),
            close,
            items = popup.list("checks"),
        )
        "PullRequestChecksFailed" -> Notice(
            stringResource(R.string.app_checks_failed_title, popup.field("number").orEmpty()),
            popup.field("title").orEmpty(),
            close,
            items = popup.list("checks"),
        )
        "GenericGitAuthentication" -> GenericGitAuthenticationDialog(
            host = popup.field("host").orEmpty(),
            initialUsername = popup.field("username"),
            onSubmit = actions::submitGenericAuth,
            onDismissRequest = close,
        )
        "PublishRepository" -> WithRepo(repo, popup, close) { id ->
            PublishRepositoryDialog(
                initialName = context.repositoryName,
                accounts = context.accounts,
                onPublish = { name, description, private, endpoint, org ->
                    actions.publishRepository(id, name, description, private, endpoint, org)
                },
                onDismissRequest = close,
            )
        }
        else -> GenericPopup(popup, close)
    }
}

/** A dialog that acts on its repository; without one it can only show its fields. */
@Composable
private fun WithRepo(repo: ULong?, popup: PopupVm, close: () -> Unit, content: @Composable (ULong) -> Unit) {
    if (repo == null) GenericPopup(popup, close) else content(repo)
}

/** GHD's `DiscardChanges`: the paths (ten, then "and n more"), Discard in red. */
@Composable
private fun ConfirmDiscardChanges(popup: PopupVm, actions: PopupActions) {
    val paths = popup.list("paths")
    val all = popup.field("all") == "true"
    val repo = popup.repo
    PrimerDialog(
        title = stringResource(if (all) R.string.app_discard_all_title else R.string.app_discard_title),
        onDismissRequest = actions::close,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                stringResource(if (all) R.string.app_discard_all_confirm else R.string.app_discard_confirm),
                { if (repo != null) actions.discardChanges(repo, paths) },
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.app_cancel), actions::close) },
    ) {
        Body(pluralStringResource(R.plurals.app_discard_body, paths.size, paths.size))
        Items(paths)
    }
}

/** GHD `PushNeedsPull`: the remote moved on. Pull (or just fetch) first. */
@Composable
private fun PushNeedsPull(onPull: () -> Unit, onFetch: () -> Unit, onClose: () -> Unit) {
    PrimerDialog(
        title = stringResource(R.string.app_push_needs_pull_title),
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.app_push_needs_pull_pull),
                onPull,
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.app_push_needs_pull_fetch), onFetch) },
    ) {
        Body(stringResource(R.string.app_push_needs_pull_body))
    }
}

/** GHD `LocalChangesOverwritten`: the files in the way; stash them, then retry. */
@Composable
private fun LocalChangesOverwritten(popup: PopupVm, onStash: () -> Unit, onRetry: () -> Unit, onClose: () -> Unit) {
    PrimerDialog(
        title = stringResource(R.string.app_overwritten_title),
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.app_retry),
                onRetry,
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.app_stash_changes), onStash) },
    ) {
        Body(stringResource(R.string.app_overwritten_body))
        Items(popup.list("files"))
        Body(stringResource(R.string.app_overwritten_hint))
    }
}

/** A confirmation: [body], an optional list, Cancel and [confirm] (red when [danger]). */
@Composable
private fun Confirm(
    title: String,
    body: String,
    confirm: String,
    danger: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    items: List<String> = emptyList(),
) {
    PrimerDialog(
        title = title,
        onDismissRequest = onDismiss,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                confirm,
                onConfirm,
                variant = if (danger) PrimerButtonVariant.Danger else PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.app_cancel), onDismiss) },
    ) {
        Body(body)
        Items(items)
    }
}

/** A dialog that only informs: [body], an optional list, Close. */
@Composable
private fun Notice(title: String, body: String, onClose: () -> Unit, items: List<String> = emptyList()) {
    PrimerDialog(
        title = title,
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.app_close),
                onClose,
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
    ) {
        if (body.isNotEmpty()) Body(body)
        Items(items)
    }
}

@Composable
private fun Body(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = CorveneTheme.colors.textPrimary)
}

@Composable
private fun Items(items: List<String>) {
    if (items.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        items.take(MAX_ITEMS).forEach { Text(it, style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textPrimary) }
        if (items.size > MAX_ITEMS) {
            Text(
                pluralStringResource(R.plurals.app_and_more, items.size - MAX_ITEMS, items.size - MAX_ITEMS),
                style = MaterialTheme.typography.bodySmall,
                color = CorveneTheme.colors.textSecondary,
            )
        }
    }
}

/** GHD's error dialog: the title, the message, and git's output behind "Show details". */
@Composable
private fun Error(popup: PopupVm, onClose: () -> Unit) {
    var details by rememberSaveable(popup) { mutableStateOf(false) }
    val output = popup.field("details") ?: popup.field("output")
    PrimerDialog(
        title = popup.field("title") ?: stringResource(R.string.app_error),
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.app_close),
                onClose,
                variant = PrimerButtonVariant.Primary,
                modifier = Modifier.testTag(TAG_POPUP_PRIMARY),
            )
        },
    ) {
        Text(
            popup.field("text") ?: popup.field("message").orEmpty(),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textPrimary,
        )
        if (!output.isNullOrBlank()) {
            PrimerButton(
                stringResource(if (details) R.string.app_hide_details else R.string.app_show_details),
                { details = !details },
                variant = PrimerButtonVariant.Link,
            )
            if (details) {
                popup.field("command")?.let {
                    Text("$ $it", style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textSecondary)
                }
                Text(
                    output,
                    Modifier.horizontalScroll(rememberScrollState()),
                    style = CorveneTheme.textStyles.codeSmall,
                    color = CorveneTheme.colors.textPrimary,
                    softWrap = false,
                )
            }
        }
    }
}

/** Any other dialog: the kind, the payload, Close. */
@Composable
private fun GenericPopup(popup: PopupVm, onClose: () -> Unit) {
    PrimerDialog(
        title = popup.kind,
        onDismissRequest = onClose,
        closeDescription = stringResource(R.string.app_close),
        confirmButton = { PrimerButton(stringResource(R.string.app_close), onClose, modifier = Modifier.testTag(TAG_POPUP_PRIMARY)) },
    ) {
        Text(
            stringResource(R.string.app_popup_later),
            style = MaterialTheme.typography.bodySmall,
            color = CorveneTheme.colors.textSecondary,
        )
        popup.fields.forEach { field ->
            Column {
                Text(field.key, style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold))
                Text(field.value, style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textPrimary)
            }
        }
        popup.lists.forEach { list ->
            Column {
                Text(list.key, style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold))
                Row { Items(list.items) }
            }
        }
    }
}

private fun PopupVm.field(key: String): String? = fields.firstOrNull { it.key == key }?.value

private fun PopupVm.list(key: String): List<String> = lists.firstOrNull { it.key == key }?.items.orEmpty()

/** `rememberCoreQuery` wants a value; this stands for "no dialog". */
private val NoPopup = PopupVm(kind = "", repo = null, fields = emptyList(), lists = emptyList())

private const val MAX_ITEMS = 10

/** The primary (or only) button of the [PopupDialog] dialogs drawn here. */
const val TAG_POPUP_PRIMARY = "app_popup_primary"
