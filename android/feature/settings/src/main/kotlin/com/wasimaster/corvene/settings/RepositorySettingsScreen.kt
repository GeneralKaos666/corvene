package com.wasimaster.corvene.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.design.UnderlineNav
import com.wasimaster.corvene.design.UnderlineNavItem
import com.wasimaster.corvene.ffi.gen.RepositorySettingsVm

/** GHD's Repository Settings tabs, by the engine's names (`openRepositorySettings(repo, key)`, the popup's `tab` field). */
enum class RepositorySettingsTab(val key: String, val popupName: String) {
    Remote("remote", "Remote"),
    IgnoredFiles("ignored", "IgnoredFiles"),
    GitConfig("git", "GitConfig"),
    ;

    companion object {
        fun fromPopup(name: String?): RepositorySettingsTab = entries.firstOrNull { it.popupName == name || it.key == name } ?: Remote
    }
}

/** What Save writes: only the parts that changed are set (`saveRepositorySettings`'s optional arguments). */
@Immutable
data class RepositorySettingsSave(
    val remoteName: String? = null,
    val remoteUrl: String? = null,
    val gitignore: String? = null,
    /** "global" or "local", with [name] and [email] for "local". */
    val gitConfigLocation: String? = null,
    val name: String? = null,
    val email: String? = null,
    /** "true" / "false". */
    val autocrlf: String? = null,
) {
    val isEmpty: Boolean get() = this == RepositorySettingsSave()
}

/**
 * Repository Settings (GHD's dialog, without Fork Behavior): Remote (the
 * primary remote's URL), Ignored files (the `.gitignore` text), Git config
 * (global or local identity, plus `core.autocrlf`). Full screen on compact
 * widths, a dialog wider. [settings] is null until the engine has read the
 * repository. Save sends what changed ([RepositorySettingsSave]) and closes.
 */
@Composable
fun RepositorySettingsDialog(
    settings: RepositorySettingsVm?,
    initialTab: RepositorySettingsTab,
    onSave: (RepositorySettingsSave) -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var tab by rememberSaveable { mutableIntStateOf(initialTab.ordinal) }
    // the edits start from the engine's values once they arrive
    var url by rememberSaveable(settings?.remoteUrl) { mutableStateOf(settings?.remoteUrl.orEmpty()) }
    var gitignore by rememberSaveable(settings?.gitignore) { mutableStateOf(settings?.gitignore.orEmpty()) }
    val localNow = settings?.localName != null || settings?.localEmail != null
    var local by rememberSaveable(settings) { mutableStateOf(localNow) }
    var name by rememberSaveable(settings) { mutableStateOf(settings?.localName ?: settings?.globalName.orEmpty()) }
    var email by rememberSaveable(settings) { mutableStateOf(settings?.localEmail ?: settings?.globalEmail.orEmpty()) }
    var autocrlf by rememberSaveable(settings) { mutableStateOf(settings?.autocrlf ?: false) }
    val save = settings?.let { vm ->
        val remoteChanged = vm.remoteName != null && url.trim() != vm.remoteUrl.orEmpty()
        val location = when {
            local && (!localNow || name != vm.localName.orEmpty() || email != vm.localEmail.orEmpty()) -> "local"
            !local && localNow -> "global"
            else -> null
        }
        RepositorySettingsSave(
            remoteName = if (remoteChanged) vm.remoteName else null,
            remoteUrl = if (remoteChanged) url.trim() else null,
            gitignore = gitignore.takeIf { it != vm.gitignore.orEmpty() },
            gitConfigLocation = location,
            name = if (location == "local") name.trim() else null,
            email = if (location == "local") email.trim() else null,
            autocrlf = if (autocrlf != vm.autocrlf) autocrlf.toString() else null,
        )
    }
    PrimerDialog(
        title = stringResource(R.string.set_repo_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        fullScreenOnCompact = true,
        dismissOnOutside = false,
        closeDescription = stringResource(R.string.set_close),
        confirmButton = {
            PrimerButton(
                stringResource(R.string.set_repo_save),
                { save?.let(onSave) },
                Modifier.testTag(TAG_REPO_SAVE),
                variant = PrimerButtonVariant.Primary,
                enabled = save != null && !save.isEmpty,
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.set_cancel), onDismissRequest, variant = PrimerButtonVariant.Invisible) },
    ) {
        UnderlineNav(
            listOf(
                UnderlineNavItem(stringResource(R.string.set_repo_remote)),
                UnderlineNavItem(stringResource(R.string.set_repo_ignored)),
                UnderlineNavItem(stringResource(R.string.set_repo_git_config)),
            ),
            selectedIndex = tab,
            onSelect = { tab = it },
            modifier = Modifier.testTag(TAG_REPO_TABS),
        )
        if (settings == null) {
            ProgressBar(null, Modifier.fillMaxWidth())
            Text(
                stringResource(R.string.set_repo_loading),
                Modifier.padding(16.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = CorveneTheme.colors.textSecondary,
            )
            return@PrimerDialog
        }
        Column(Modifier.padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            when (RepositorySettingsTab.entries[tab]) {
                RepositorySettingsTab.Remote -> RemoteTab(settings, url) { url = it }
                RepositorySettingsTab.IgnoredFiles -> {
                    Text(
                        stringResource(R.string.set_repo_ignored_body),
                        style = MaterialTheme.typography.bodySmall,
                        color = CorveneTheme.colors.textSecondary,
                    )
                    PrimerTextField(
                        gitignore,
                        { gitignore = it },
                        Modifier.fillMaxWidth().heightIn(min = 200.dp).testTag(TAG_REPO_GITIGNORE),
                        label = stringResource(R.string.set_repo_gitignore),
                        singleLine = false,
                        minLines = 8,
                        monospace = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii),
                    )
                }
                RepositorySettingsTab.GitConfig -> {
                    Column(Modifier.selectableGroup()) {
                        RadioRow(
                            stringResource(R.string.set_repo_use_global),
                            selected = !local,
                            onClick = {
                                local = false
                                name = settings.globalName.orEmpty()
                                email = settings.globalEmail.orEmpty()
                            },
                            modifier = Modifier.testTag(TAG_REPO_GLOBAL),
                        )
                        RadioRow(
                            stringResource(R.string.set_repo_use_local),
                            selected = local,
                            onClick = { local = true },
                            modifier = Modifier.testTag(TAG_REPO_LOCAL),
                        )
                    }
                    PrimerTextField(
                        name,
                        { name = it },
                        Modifier.fillMaxWidth().testTag(TAG_REPO_NAME),
                        label = stringResource(R.string.set_git_name),
                        enabled = local,
                    )
                    PrimerTextField(
                        email,
                        { email = it },
                        Modifier.fillMaxWidth().testTag(TAG_REPO_EMAIL),
                        label = stringResource(R.string.set_git_email),
                        enabled = local,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                    )
                    SwitchRow(
                        stringResource(R.string.set_repo_autocrlf),
                        autocrlf,
                        { autocrlf = it },
                        Modifier.testTag(TAG_REPO_AUTOCRLF),
                        caption = stringResource(R.string.set_repo_autocrlf_body),
                    )
                }
            }
        }
    }
}

@Composable
private fun RemoteTab(settings: RepositorySettingsVm, url: String, onUrl: (String) -> Unit) {
    val remote = settings.remoteName
    if (remote == null) {
        Text(
            stringResource(R.string.set_repo_no_remote),
            Modifier.testTag(TAG_REPO_NO_REMOTE),
            style = MaterialTheme.typography.bodyMedium,
            color = CorveneTheme.colors.textSecondary,
        )
        return
    }
    Text(
        stringResource(R.string.set_repo_remote_name, remote),
        style = MaterialTheme.typography.titleSmall,
        color = CorveneTheme.colors.textPrimary,
    )
    PrimerTextField(
        url,
        onUrl,
        Modifier.fillMaxWidth().testTag(TAG_REPO_URL),
        label = stringResource(R.string.set_repo_remote_url),
        monospace = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
    )
}

const val TAG_REPO_SAVE = "set_repo_save"
const val TAG_REPO_TABS = "set_repo_tabs"
const val TAG_REPO_URL = "set_repo_url"
const val TAG_REPO_NO_REMOTE = "set_repo_no_remote"
const val TAG_REPO_GITIGNORE = "set_repo_gitignore"
const val TAG_REPO_GLOBAL = "set_repo_global"
const val TAG_REPO_LOCAL = "set_repo_local"
const val TAG_REPO_NAME = "set_repo_name"
const val TAG_REPO_EMAIL = "set_repo_email"
const val TAG_REPO_AUTOCRLF = "set_repo_autocrlf"
