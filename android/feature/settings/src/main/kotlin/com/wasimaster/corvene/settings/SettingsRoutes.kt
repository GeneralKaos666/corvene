package com.wasimaster.corvene.settings

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.ffi.InstalledApps
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.CoreException
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.FolderResolver
import com.wasimaster.corvene.platform.HostState
import com.wasimaster.corvene.platform.Notifications
import com.wasimaster.corvene.platform.Termux
import com.wasimaster.corvene.platform.openNotificationSettings
import com.wasimaster.corvene.platform.openUrl
import com.wasimaster.corvene.platform.rememberNotificationPermission
import com.wasimaster.corvene.platform.requestAllFilesAccess
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * One Settings section wired to the engine: `settings()` in, `setSetting`
 * out, plus each section's own queries and Android's side (installed
 * editors, notification permission, all-files access, re-read whenever the
 * app comes back from Android's settings). [version] is the app's
 * versionName for About; [onSignIn] opens the sign-in destination,
 * [onLicenses] the licences page.
 */
@Composable
fun SettingsSectionRoute(
    section: SettingsSection,
    version: String,
    onSignIn: (enterprise: Boolean) -> Unit,
    onLicenses: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    when (section) {
        SettingsSection.Accounts -> AccountsRoute(onSignIn, modifier, contentPadding)
        SettingsSection.Appearance -> AppearanceRoute(modifier, contentPadding)
        SettingsSection.Flags -> FlagsRoute(modifier, contentPadding)
        SettingsSection.About -> {
            val context = LocalContext.current
            AboutScreen(version, onLicenses, onLink = { openUrl(context, it) }, modifier, contentPadding)
        }
        SettingsSection.Git -> GitRoute(modifier, contentPadding)
        SettingsSection.Integrations, SettingsSection.Notifications, SettingsSection.Prompts, SettingsSection.Advanced,
        SettingsSection.Accessibility,
        -> EngineSettingsRoute(section, modifier, contentPadding)
    }
}

/** The sections that are plain settings: Integrations, Notifications, Prompts, Advanced, Accessibility. */
@Composable
private fun EngineSettingsRoute(section: SettingsSection, modifier: Modifier, contentPadding: PaddingValues) {
    val core = LocalCore.current
    val context = LocalContext.current
    val settings by rememberCoreQuery { settings() }
    // Android's answers change behind the app's back (its settings pages): read them again on every resume
    var resumed by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { resumed++ }
    val writer = remember(core) { SettingWriter { key, value -> core.dispatch { setSetting(key, value) } } }
    val vm = settings.value ?: return Loading(modifier)
    when (section) {
        SettingsSection.Integrations -> {
            val editors by produceState(emptyList<EditorApp>(), resumed) {
                value = withContext(Dispatchers.IO) { EditorApp.parse(InstalledApps.textViewers(context)) }
            }
            val termux = remember(resumed) { Termux.installed(context) }
            IntegrationsScreen(vm, editors, termux, writer, modifier, contentPadding)
        }
        SettingsSection.Notifications -> {
            var allowed by remember(resumed) { mutableStateOf(Notifications.allowed(context)) }
            val ask = rememberNotificationPermission { granted ->
                allowed = granted
                HostState.refresh(core, context)
            }
            NotificationsScreen(
                vm,
                systemAllowed = allowed,
                onEnabled = { on ->
                    writer.set(SettingKey.NOTIFICATIONS_ENABLED, on.toString())
                    if (on && !allowed) ask()
                },
                onOpenSystemSettings = { openNotificationSettings(context) },
                modifier = modifier,
                contentPadding = contentPadding,
            )
        }
        SettingsSection.Advanced -> {
            val allFiles = remember(resumed) {
                if (FolderResolver.canRequestAllFilesAccess(context)) AllFilesAccess(FolderResolver.hasAllFilesAccess(context)) else null
            }
            AdvancedScreen(vm, allFiles, writer, onAllFiles = { requestAllFilesAccess(context) }, modifier, contentPadding)
        }
        SettingsSection.Accessibility -> AccessibilityScreen(vm, writer, modifier, contentPadding)
        SettingsSection.Prompts -> PromptsScreen(vm, writer, modifier, contentPadding)
        SettingsSection.Accounts, SettingsSection.Git, SettingsSection.Appearance, SettingsSection.Flags, SettingsSection.About -> Unit
    }
}

/** Settings › Git: `gitIdentity()` and `globalGitConfig()` in, `setGlobalIdentity` and `openGlobalGitConfig` out. */
@Composable
private fun GitRoute(modifier: Modifier, contentPadding: PaddingValues) {
    val core = LocalCore.current
    val identity by rememberCoreQuery { gitIdentity() }
    val global by rememberCoreQuery { globalGitConfig() }
    if (identity.loading) return Loading(modifier)
    GitScreen(
        name = identity.value?.name.orEmpty(),
        email = identity.value?.email.orEmpty(),
        // the engine reads the global config's init.defaultBranch only for its Preferences popup; git's own default otherwise
        defaultBranch = global.value?.defaultBranch ?: DEFAULT_BRANCH,
        onSave = { name, email -> core.dispatch { setGlobalIdentity(name, email) } },
        onEditConfig = { core.dispatch { openGlobalGitConfig() } },
        modifier = modifier,
        contentPadding = contentPadding,
    )
}

/**
 * Settings › Flags wired to the engine: `flags()` in; `setFlagBySlug`
 * (whose refusal shows under the flag), `resetFlag`, `applyPreset`,
 * `resetAllFlags`, `relaunch` out.
 */
@Composable
fun FlagsRoute(modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val scope = rememberCoroutineScope()
    val flags by rememberCoreQuery { flags() }
    var view by rememberSaveable(stateSaver = FlagsViewSaver) { mutableStateOf(FlagsView()) }
    var errors by remember { mutableStateOf(emptyMap<String, String>()) }
    val actions = remember(core) {
        object : FlagActions {
            override fun set(slug: String, value: String) {
                scope.launch {
                    errors = try {
                        core.query { setFlagBySlug(slug, value) }
                        errors - slug
                    } catch (refused: CoreException) {
                        errors + (slug to ((refused as? CoreException.Failed)?.reason ?: refused.message.orEmpty()))
                    }
                }
            }

            override fun reset(slug: String) {
                errors = errors - slug
                core.dispatch { resetFlag(slug) }
            }

            override fun applyPreset(slug: String) {
                errors = emptyMap()
                core.dispatch { applyPreset(slug) }
            }

            override fun resetAll() {
                errors = emptyMap()
                core.dispatch { resetAllFlags() }
            }

            override fun relaunch() = core.dispatch { relaunch() }
        }
    }
    val value = flags.value ?: return Loading(modifier)
    FlagsScreen(value, view, { view = it }, errors, actions, modifier, contentPadding)
}

private val FlagsViewSaver = androidx.compose.runtime.saveable.Saver<FlagsView, List<Any>>(
    save = { listOf(it.query, it.filter.ordinal, it.showBugFixes) },
    restore = { FlagsView(it[0] as String, FlagFilter.entries[it[1] as Int], it[2] as Boolean) },
)

/**
 * Repository Settings over the engine's `RepositorySettings` popup:
 * `repositorySettings()` (read in the background after
 * `openRepositorySettings`), Save = `saveRepositorySettings` (which closes
 * the popup), Cancel = [onClose].
 */
@Composable
fun RepositorySettingsRoute(repo: Long, tab: String?, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val settings by rememberCoreQuery(repo) { repositorySettings()?.takeIf { it.repo == repo.toULong() } }
    RepositorySettingsDialog(
        settings.value,
        RepositorySettingsTab.fromPopup(tab),
        onSave = { save ->
            core.dispatch {
                saveRepositorySettings(
                    repo.toULong(),
                    save.remoteName,
                    save.remoteUrl,
                    save.gitignore,
                    save.gitConfigLocation,
                    save.name,
                    save.email,
                    save.autocrlf,
                )
            }
        },
        onDismissRequest = onClose,
        modifier = modifier,
    )
}

/** The licences page: the NOTICE files :app packs as assets (`notices/`). */
@Composable
fun LicensesRoute(modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val context = LocalContext.current
    val text by produceState<String?>(null) {
        value = withContext(Dispatchers.IO) {
            val names = context.assets.list(NOTICES).orEmpty().sorted()
            names.joinToString("\n\n") { name -> context.assets.open("$NOTICES/$name").bufferedReader().use { it.readText() } }
        }
    }
    val value = text ?: return Loading(modifier)
    LicensesScreen(value, modifier, contentPadding)
}

@Composable
private fun Loading(modifier: Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) { ProgressBar(null) }
}

private const val DEFAULT_BRANCH = "main"
private const val NOTICES = "notices"
