package com.wasimaster.corvene.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.ffi.gen.SettingsVm

/** The engine's `setSetting` keys (snake_case, as `crates/corvene-ffi/src/api.rs::set_setting` reads them). */
object SettingKey {
    const val CONFIRM_DISCARD_CHANGES = "confirm_discard_changes"
    const val CONFIRM_CHECKOUT_COMMIT = "confirm_checkout_commit"
    const val CONFIRM_UNDO_COMMIT = "confirm_undo_commit"
    const val CONFIRM_DISCARD_STASH = "confirm_discard_stash"
    const val CONFIRM_FORCE_PUSH = "confirm_force_push"
    const val CONFIRM_REPOSITORY_REMOVAL = "confirm_repository_removal"
    const val CONFIRM_COMMIT_FILTERED_CHANGES = "confirm_commit_filtered_changes"
    const val NOTIFICATIONS_ENABLED = "notifications_enabled"
    const val REPOSITORY_INDICATORS_ENABLED = "repository_indicators_enabled"
    const val SHOW_DIFF_CHECK_MARKS = "show_diff_check_marks"
    const val UNDERLINE_LINKS = "underline_links"
    const val SHOW_COMMIT_LENGTH_WARNING = "show_commit_length_warning"
    const val UNCOMMITTED_CHANGES_STRATEGY = "uncommitted_changes_strategy"
    const val EXTERNAL_EDITOR = "external_editor"
}

/** Writes one setting: a key of [SettingKey] and "true"/"false" or the text ("" clears a text setting). */
fun interface SettingWriter {
    fun set(key: String, value: String)
}

private fun SettingWriter.toggle(key: String): (Boolean) -> Unit = { set(key, it.toString()) }

/** A section's scrolling body over the canvas, with the gutter's padding at the bottom. */
@Composable
internal fun SettingsPage(modifier: Modifier, contentPadding: PaddingValues, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .fillMaxSize()
            .background(CorveneTheme.colors.bgCanvas)
            .verticalScroll(rememberScrollState())
            .padding(contentPadding)
            .padding(bottom = 24.dp),
        content = content,
    )
}

/** A paragraph under a group header, in the secondary colour. */
@Composable
internal fun SettingsNote(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 4.dp),
        style = MaterialTheme.typography.bodySmall,
        color = CorveneTheme.colors.textSecondary,
    )
}

/** An external editor the engine can open files with: Android's label (what `external_editor` stores) and "package/class". */
data class EditorApp(val label: String, val component: String) {
    companion object {
        /** `viewApps()`'s "label\tpackage/class" lines. */
        fun parse(lines: List<String>): List<EditorApp> = lines.mapNotNull { line ->
            val parts = line.split('\t', limit = 2)
            if (parts.size == 2 && parts[0].isNotBlank()) EditorApp(parts[0], parts[1]) else null
        }.distinctBy { it.label }
    }
}

/**
 * Settings › Integrations: the external editor (GHD's popup, here a radio
 * list of the apps that open text files, the engine's `viewApps()`), and the
 * shell, which on Android is Termux (fixed: no other terminal takes a
 * working directory).
 */
@Composable
fun IntegrationsScreen(
    settings: SettingsVm,
    editors: List<EditorApp>,
    termuxInstalled: Boolean,
    writer: SettingWriter,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        ActionListGroupHeader(stringResource(R.string.set_editor))
        Column(Modifier.selectableGroup()) {
            RadioRow(
                stringResource(R.string.set_editor_none),
                selected = settings.externalEditor == null,
                onClick = { writer.set(SettingKey.EXTERNAL_EDITOR, "") },
                caption = stringResource(R.string.set_editor_none_body),
                modifier = Modifier.testTag("${TAG_EDITOR}none"),
            )
            editors.forEach { editor ->
                RadioRow(
                    editor.label,
                    selected = settings.externalEditor == editor.label,
                    onClick = { writer.set(SettingKey.EXTERNAL_EDITOR, editor.label) },
                    caption = editor.component.substringBefore('/'),
                    modifier = Modifier.testTag("$TAG_EDITOR${editor.label}"),
                )
            }
        }
        if (editors.isEmpty()) SettingsNote(stringResource(R.string.set_editor_empty))
        ActionListGroupHeader(stringResource(R.string.set_shell), Modifier.padding(top = 8.dp))
        ActionListItem(
            stringResource(R.string.set_shell_termux),
            description = stringResource(if (termuxInstalled) R.string.set_shell_termux_installed else R.string.set_shell_termux_missing),
            checked = termuxInstalled,
            leading = { Octicon(Octicons.Terminal, null) },
        )
    }
}

/**
 * Settings › Git (GHD's Author and Default branch sub-tabs): name and email
 * written to the global config with Save, the default branch new
 * repositories start with (read-only: the FFI has no setter yet), and the
 * global config file in the external editor.
 */
@Composable
fun GitScreen(
    name: String,
    email: String,
    defaultBranch: String,
    onSave: (name: String, email: String) -> Unit,
    onEditConfig: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    var editedName by rememberSaveable(name) { mutableStateOf(name) }
    var editedEmail by rememberSaveable(email) { mutableStateOf(email) }
    val changed = editedName.trim() != name.trim() || editedEmail.trim() != email.trim()
    SettingsPage(modifier.imePadding(), contentPadding) {
        Flash(
            stringResource(R.string.set_git_note),
            icon = Octicons.Info,
            modifier = Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
        )
        ActionListGroupHeader(stringResource(R.string.set_git_author))
        Column(
            Modifier.padding(horizontal = CorveneTheme.metrics.gutter),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            PrimerTextField(
                editedName,
                { editedName = it },
                Modifier.fillMaxWidth().testTag(TAG_GIT_NAME),
                label = stringResource(R.string.set_git_name),
            )
            PrimerTextField(
                editedEmail,
                { editedEmail = it },
                Modifier.fillMaxWidth().testTag(TAG_GIT_EMAIL),
                label = stringResource(R.string.set_git_email),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                PrimerButton(
                    stringResource(R.string.set_git_save),
                    { onSave(editedName.trim(), editedEmail.trim()) },
                    Modifier.testTag(TAG_GIT_SAVE),
                    variant = PrimerButtonVariant.Primary,
                    enabled = changed && editedName.isNotBlank() && editedEmail.isNotBlank(),
                )
            }
        }
        ActionListGroupHeader(stringResource(R.string.set_git_default_branch), Modifier.padding(top = 8.dp))
        ActionListItem(
            defaultBranch,
            description = stringResource(R.string.set_git_default_branch_body, defaultBranch),
            leading = { Octicon(Octicons.GitBranch, null) },
        )
        ActionListDivider()
        ActionListItem(
            stringResource(R.string.set_git_edit_config),
            onClick = onEditConfig,
            leading = { Octicon(Octicons.Pencil, null, tint = OcticonTint.Link) },
            modifier = Modifier.testTag(TAG_GIT_EDIT_CONFIG),
        )
    }
}

/**
 * Settings › Notifications: GHD's "Enable notifications" (the engine's
 * setting), what Android allows ([systemAllowed]: POST_NOTIFICATIONS on 13+,
 * the app switch below), and the channels page.
 */
@Composable
fun NotificationsScreen(
    settings: SettingsVm,
    systemAllowed: Boolean,
    onEnabled: (Boolean) -> Unit,
    onOpenSystemSettings: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        SwitchRow(
            stringResource(R.string.set_notifications_enable),
            checked = settings.notificationsEnabled,
            onCheckedChange = onEnabled,
            caption = stringResource(R.string.set_notifications_enable_body),
            modifier = Modifier.testTag("$TAG_SETTING${SettingKey.NOTIFICATIONS_ENABLED}"),
        )
        if (settings.notificationsEnabled && !systemAllowed) {
            Flash(
                stringResource(R.string.set_notifications_blocked),
                variant = FlashVariant.Warning,
                modifier = Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp).testTag(TAG_NOTIFICATIONS_BLOCKED),
                action = {
                    PrimerButton(
                        stringResource(R.string.set_notifications_open_settings),
                        onOpenSystemSettings,
                        variant = PrimerButtonVariant.Link,
                    )
                },
            )
        }
        ActionListDivider()
        ActionListItem(
            stringResource(R.string.set_notifications_channels),
            description = stringResource(R.string.set_notifications_channels_body),
            onClick = onOpenSystemSettings,
            leading = { Octicon(Octicons.Bell, null) },
            trailing = { Octicon(Octicons.LinkExternal, null, tint = OcticonTint.Secondary) },
        )
    }
}

/**
 * Settings › Prompts, as GHD 3.6.6 has it: the confirmations, what happens
 * to changes when switching branches, the commit length warning.
 * ("Discarding changes permanently" waits for its value in the view model.)
 */
@Composable
fun PromptsScreen(
    settings: SettingsVm,
    writer: SettingWriter,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        ActionListGroupHeader(stringResource(R.string.set_prompts_confirm))
        listOf(
            Triple(R.string.set_prompt_remove_repository, SettingKey.CONFIRM_REPOSITORY_REMOVAL, settings.confirmRepositoryRemoval),
            Triple(R.string.set_prompt_discard, SettingKey.CONFIRM_DISCARD_CHANGES, settings.confirmDiscardChanges),
            Triple(R.string.set_prompt_discard_stash, SettingKey.CONFIRM_DISCARD_STASH, settings.confirmDiscardStash),
            Triple(R.string.set_prompt_checkout_commit, SettingKey.CONFIRM_CHECKOUT_COMMIT, settings.confirmCheckoutCommit),
            Triple(R.string.set_prompt_force_push, SettingKey.CONFIRM_FORCE_PUSH, settings.confirmForcePush),
            Triple(R.string.set_prompt_undo_commit, SettingKey.CONFIRM_UNDO_COMMIT, settings.confirmUndoCommit),
            Triple(R.string.set_prompt_filtered, SettingKey.CONFIRM_COMMIT_FILTERED_CHANGES, settings.confirmCommitFilteredChanges),
        ).forEach { (label, key, value) ->
            SwitchRow(stringResource(label), value, writer.toggle(key), Modifier.testTag("$TAG_SETTING$key"))
        }
        ActionListGroupHeader(stringResource(R.string.set_strategy), Modifier.padding(top = 8.dp))
        Column(Modifier.selectableGroup()) {
            listOf(
                "ask" to R.string.set_strategy_ask,
                "move" to R.string.set_strategy_move,
                "stash" to R.string.set_strategy_stash,
            ).forEach { (value, label) ->
                RadioRow(
                    stringResource(label),
                    selected = settings.uncommittedChangesStrategy == value,
                    onClick = { writer.set(SettingKey.UNCOMMITTED_CHANGES_STRATEGY, value) },
                    modifier = Modifier.testTag("$TAG_STRATEGY$value"),
                )
            }
        }
        ActionListGroupHeader(stringResource(R.string.set_commit_length), Modifier.padding(top = 8.dp))
        SwitchRow(
            stringResource(R.string.set_commit_length_warning),
            settings.showCommitLengthWarning,
            writer.toggle(SettingKey.SHOW_COMMIT_LENGTH_WARNING),
            Modifier.testTag("$TAG_SETTING${SettingKey.SHOW_COMMIT_LENGTH_WARNING}"),
        )
    }
}

/**
 * Settings › Advanced: GHD's background updates (status icons in the
 * repository list) and, where Android lets an app ask for it (the foss
 * build), All files access, which decides whether shared-storage
 * repositories open in place. ("Use Git Credential Manager" waits for its
 * value in the view model; usage stats are omitted, Corvene has none.)
 */
@Composable
fun AdvancedScreen(
    settings: SettingsVm,
    allFiles: AllFilesAccess?,
    writer: SettingWriter,
    onAllFiles: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        ActionListGroupHeader(stringResource(R.string.set_background_updates))
        SwitchRow(
            stringResource(R.string.set_indicators),
            settings.repositoryIndicatorsEnabled,
            writer.toggle(SettingKey.REPOSITORY_INDICATORS_ENABLED),
            Modifier.testTag("$TAG_SETTING${SettingKey.REPOSITORY_INDICATORS_ENABLED}"),
            caption = stringResource(R.string.set_indicators_body),
        )
        if (allFiles != null) {
            ActionListGroupHeader(stringResource(R.string.set_storage), Modifier.padding(top = 8.dp))
            ActionListItem(
                stringResource(R.string.set_all_files),
                description = stringResource(if (allFiles.granted) R.string.set_all_files_granted else R.string.set_all_files_missing),
                onClick = onAllFiles,
                modifier = Modifier.testTag(TAG_ALL_FILES),
                leading = { Octicon(Octicons.FileDirectory, null) },
                trailing = {
                    Octicon(
                        if (allFiles.granted) Octicons.CheckCircleFill else Octicons.LinkExternal,
                        null,
                        tint = if (allFiles.granted) OcticonTint.Success else OcticonTint.Secondary,
                    )
                },
            )
        }
    }
}

/** All files access (MANAGE_EXTERNAL_STORAGE): offered by the foss build on Android 11+. */
data class AllFilesAccess(val granted: Boolean)

/** Settings › Accessibility: underline links, check marks in the diff. */
@Composable
fun AccessibilityScreen(
    settings: SettingsVm,
    writer: SettingWriter,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        SwitchRow(
            stringResource(R.string.set_underline_links),
            settings.underlineLinks,
            writer.toggle(SettingKey.UNDERLINE_LINKS),
            Modifier.testTag("$TAG_SETTING${SettingKey.UNDERLINE_LINKS}"),
            caption = stringResource(R.string.set_underline_links_body),
        )
        SwitchRow(
            stringResource(R.string.set_check_marks),
            settings.showDiffCheckMarks,
            writer.toggle(SettingKey.SHOW_DIFF_CHECK_MARKS),
            Modifier.testTag("$TAG_SETTING${SettingKey.SHOW_DIFF_CHECK_MARKS}"),
            caption = stringResource(R.string.set_check_marks_body),
        )
    }
}

/** Settings › About: the version, the licences ([onLicenses]), the source and issue links ([onLink]). */
@Composable
fun AboutScreen(
    version: String,
    onLicenses: () -> Unit,
    onLink: (String) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    SettingsPage(modifier, contentPadding) {
        Column(
            Modifier.fillMaxWidth().padding(vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Octicon(Octicons.MarkGithub, null, size = 48.dp)
            Text("Corvene", style = MaterialTheme.typography.titleLarge, color = CorveneTheme.colors.textPrimary)
            Text(
                stringResource(R.string.set_based_on),
                Modifier.padding(horizontal = 32.dp),
                style = MaterialTheme.typography.bodySmall,
                color = CorveneTheme.colors.textSecondary,
            )
        }
        ActionListItem(stringResource(R.string.set_version), description = version, modifier = Modifier.testTag(TAG_VERSION))
        ActionListDivider()
        ActionListItem(
            stringResource(R.string.set_licenses),
            description = stringResource(R.string.set_licenses_body),
            onClick = onLicenses,
            chevron = true,
            modifier = Modifier.testTag(TAG_LICENSES),
            leading = { Octicon(Octicons.Law, null) },
        )
        ActionListDivider()
        ActionListItem(
            stringResource(R.string.set_source),
            description = SOURCE_URL.removePrefix("https://"),
            onClick = { onLink(SOURCE_URL) },
            leading = { Octicon(Octicons.Repo, null) },
            trailing = { Octicon(Octicons.LinkExternal, null, tint = OcticonTint.Secondary) },
        )
        ActionListDivider()
        ActionListItem(
            stringResource(R.string.set_report_issue),
            onClick = { onLink("$SOURCE_URL/issues") },
            leading = { Octicon(Octicons.IssueOpened, null) },
            trailing = { Octicon(Octicons.LinkExternal, null, tint = OcticonTint.Secondary) },
        )
    }
}

/** The licence texts (the repository's NOTICE and the Android app's), monospaced. */
@Composable
fun LicensesScreen(text: String, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    SettingsPage(modifier, contentPadding) {
        Text(
            text,
            Modifier.padding(CorveneTheme.metrics.gutter).testTag(TAG_LICENSES_TEXT),
            style = CorveneTheme.textStyles.codeSmall,
            color = CorveneTheme.colors.textPrimary,
        )
    }
}

const val SOURCE_URL = "https://github.com/wasi-master/corvene"
const val TAG_SETTING = "set_setting_"
const val TAG_STRATEGY = "set_strategy_"
const val TAG_EDITOR = "set_editor_"
const val TAG_GIT_NAME = "set_git_name"
const val TAG_GIT_EMAIL = "set_git_email"
const val TAG_GIT_SAVE = "set_git_save"
const val TAG_GIT_EDIT_CONFIG = "set_git_edit_config"
const val TAG_NOTIFICATIONS_BLOCKED = "set_notifications_blocked"
const val TAG_ALL_FILES = "set_all_files"
const val TAG_VERSION = "set_version"
const val TAG_LICENSES = "set_licenses"
const val TAG_LICENSES_TEXT = "set_licenses_text"
