package com.wasimaster.corvene.branches

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.common.relativeTime
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuDivider
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.design.SyncButtonModel
import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.SyncActionVm

/** What the push/pull button and its menu send. */
interface SyncActions {
    fun fetch()

    fun pull()

    fun push()

    /** `push(repo, forceWithLease = true)`, after [ConfirmForcePushDialog] when the setting asks. */
    fun forcePush()

    /** Opens the Publish Repository form ([PublishRepositoryDialog]). */
    fun publishRepository()
}

/**
 * GHD's `PushPullButton` from the engine's sync state: "Fetch origin ·
 * Last fetched 5 min. ago", "Pull origin 2↓", "Push origin 1↑", "Publish
 * branch", "Publish repository", or the running operation's title with its
 * progress.
 */
@Composable
fun syncButtonModel(branches: BranchesVm?, now: Long = System.currentTimeMillis()): SyncButtonModel {
    val fetched = branches?.lastFetchedAt?.let { stringResource(R.string.br_sync_fetched, relativeTime(it, now)) }
        ?: stringResource(R.string.br_sync_never)
    return when (branches?.sync) {
        null -> SyncButtonModel(stringResource(R.string.br_sync_fetch), fetched, Octicons.Sync, enabled = false)
        SyncActionVm.FETCH -> SyncButtonModel(stringResource(R.string.br_sync_fetch), fetched, Octicons.Sync, enabled = true)
        SyncActionVm.PULL ->
            SyncButtonModel(stringResource(R.string.br_sync_pull, (branches.behind ?: 0u).toInt()), fetched, Octicons.ArrowDown, true)
        SyncActionVm.PUSH ->
            SyncButtonModel(stringResource(R.string.br_sync_push, (branches.ahead ?: 0u).toInt()), fetched, Octicons.ArrowUp, true)
        SyncActionVm.PUBLISH_REPOSITORY -> SyncButtonModel(
            stringResource(R.string.br_sync_publish_repository),
            stringResource(R.string.br_sync_publish_repository_caption),
            Octicons.Upload,
            true,
        )
        SyncActionVm.PUBLISH_BRANCH -> SyncButtonModel(
            stringResource(R.string.br_sync_publish_branch),
            stringResource(R.string.br_sync_publish_branch_caption),
            Octicons.Upload,
            true,
        )
        SyncActionVm.BUSY -> SyncButtonModel(
            branches.syncProgressTitle ?: stringResource(R.string.br_sync_busy),
            stringResource(R.string.br_sync_busy_caption),
            Octicons.Sync,
            enabled = false,
            progress = branches.syncProgress ?: 0f,
        )
    }
}

/** The button's tap: what [SyncActionVm] says the next step is. */
fun SyncActions.primary(sync: SyncActionVm?) {
    when (sync) {
        SyncActionVm.FETCH -> fetch()
        SyncActionVm.PULL -> pull()
        SyncActionVm.PUSH, SyncActionVm.PUBLISH_BRANCH -> push()
        SyncActionVm.PUBLISH_REPOSITORY -> publishRepository()
        SyncActionVm.BUSY, null -> Unit
    }
}

/**
 * GHD's push/pull dropdown, on a long press: Fetch, Pull, Push, Force push
 * with lease (there is an upstream to overwrite), Publish repository (no remote).
 */
@Composable
fun SyncMenu(expanded: Boolean, branches: BranchesVm?, actions: SyncActions, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    val sync = branches?.sync
    val idle = sync != null && sync != SyncActionVm.BUSY
    val tracked = sync == SyncActionVm.FETCH || sync == SyncActionVm.PULL || sync == SyncActionVm.PUSH
    fun run(action: () -> Unit): () -> Unit = {
        onDismissRequest()
        action()
    }
    ActionMenu(expanded = expanded, onDismissRequest = onDismissRequest, modifier = modifier) {
        if (sync == SyncActionVm.PUBLISH_REPOSITORY) {
            ActionMenuItem(
                stringResource(R.string.br_sync_publish_repository),
                run(actions::publishRepository),
                leadingIcon = Octicons.Upload,
            )
            return@ActionMenu
        }
        ActionMenuItem(
            stringResource(R.string.br_sync_fetch),
            run(actions::fetch),
            leadingIcon = Octicons.Sync,
            enabled = idle,
            modifier = Modifier.testTag(TAG_SYNC_FETCH),
        )
        ActionMenuItem(
            stringResource(R.string.br_sync_pull_plain),
            run(actions::pull),
            leadingIcon = Octicons.ArrowDown,
            enabled = idle && tracked && (branches?.behind ?: 0u) > 0u,
        )
        ActionMenuItem(
            stringResource(if (tracked) R.string.br_sync_push_plain else R.string.br_sync_publish_branch),
            run(actions::push),
            leadingIcon = Octicons.ArrowUp,
            enabled = idle && (!tracked || (branches?.ahead ?: 0u) > 0u),
        )
        ActionMenuDivider()
        ActionMenuItem(
            stringResource(R.string.br_sync_force_push),
            run(actions::forcePush),
            leadingIcon = Octicons.RepoPush,
            danger = true,
            enabled = idle && tracked,
            modifier = Modifier.testTag(TAG_SYNC_FORCE),
        )
    }
}

/** GHD `ConfirmForcePush`. */
@Composable
fun ConfirmForcePushDialog(upstream: String, onForcePush: () -> Unit, onDismissRequest: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.br_force_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_force_confirm),
                onForcePush,
                variant = PrimerButtonVariant.Danger,
                modifier = Modifier.testTag(TAG_FORCE_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_force_body, upstream))
        Body(stringResource(R.string.br_force_body_2))
    }
}

/**
 * GHD `PublishRepository`: name, description, private, the account
 * ([accounts], GitHub.com or an Enterprise) and an optional organization.
 * Without an account the form says to sign in first.
 */
@Composable
fun PublishRepositoryDialog(
    initialName: String,
    accounts: List<AccountVm>,
    onPublish: (name: String, description: String, private: Boolean, endpoint: String, org: String?) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by rememberSaveable { mutableStateOf(initialName) }
    var description by rememberSaveable { mutableStateOf("") }
    var private by rememberSaveable { mutableStateOf(true) }
    var org by rememberSaveable { mutableStateOf("") }
    var endpoint by rememberSaveable { mutableStateOf(accounts.firstOrNull()?.endpoint) }
    PrimerDialog(
        title = stringResource(R.string.br_publish_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            val chosen = endpoint
            PrimerButton(
                stringResource(R.string.br_publish_confirm),
                { if (chosen != null) onPublish(name.trim(), description.trim(), private, chosen, org.trim().ifEmpty { null }) },
                variant = PrimerButtonVariant.Primary,
                enabled = chosen != null && name.isNotBlank(),
                modifier = Modifier.testTag(TAG_PUBLISH_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        if (accounts.isEmpty()) {
            Text(
                stringResource(R.string.br_publish_sign_in),
                style = MaterialTheme.typography.bodyMedium,
                color = CorveneTheme.colors.attention.fg,
            )
        }
        accounts.forEach { account ->
            RadioRow(
                account.host,
                selected = endpoint == account.endpoint,
                onClick = { endpoint = account.endpoint },
                caption = "@${account.login}",
            )
        }
        PrimerTextField(name, { name = it }, label = stringResource(R.string.br_publish_name))
        PrimerTextField(description, { description = it }, label = stringResource(R.string.br_publish_description))
        SwitchRow(stringResource(R.string.br_publish_private), private, { private = it })
        PrimerTextField(
            org,
            { org = it },
            label = stringResource(R.string.br_publish_org),
            placeholder = stringResource(R.string.br_publish_org_none),
        )
    }
}

/**
 * GHD `GenericGitAuthentication`: a username and password (or token) for a
 * non-GitHub remote; Save stores them and retries what failed.
 */
@Composable
fun GenericGitAuthenticationDialog(
    host: String,
    initialUsername: String?,
    onSubmit: (username: String, password: String) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var username by rememberSaveable { mutableStateOf(initialUsername.orEmpty()) }
    // never saved across process death
    var password by remember { mutableStateOf("") }
    PrimerDialog(
        title = stringResource(R.string.br_auth_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.br_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.br_auth_confirm),
                { onSubmit(username.trim(), password) },
                variant = PrimerButtonVariant.Primary,
                enabled = username.isNotBlank() && password.isNotEmpty(),
                modifier = Modifier.testTag(TAG_AUTH_CONFIRM),
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.br_cancel), onDismissRequest) },
    ) {
        Body(stringResource(R.string.br_auth_body, host))
        PrimerTextField(username, { username = it }, label = stringResource(R.string.br_auth_username))
        PrimerTextField(
            password,
            { password = it },
            label = stringResource(R.string.br_auth_password),
            password = true,
        )
        Text(
            stringResource(R.string.br_auth_token_hint),
            style = MaterialTheme.typography.bodySmall,
            color = CorveneTheme.colors.textSecondary,
        )
    }
}

const val TAG_SYNC_FETCH = "br_sync_fetch"
const val TAG_SYNC_FORCE = "br_sync_force"
const val TAG_FORCE_CONFIRM = "br_force_confirm"
const val TAG_PUBLISH_CONFIRM = "br_publish_confirm"
const val TAG_AUTH_CONFIRM = "br_auth_confirm"
