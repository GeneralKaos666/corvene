package com.wasimaster.corvene.repositories

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.gen.CloneProgressVm
import com.wasimaster.corvene.ffi.gen.CloneableRepositoryVm
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.CorveneDocumentsProvider
import com.wasimaster.corvene.platform.FolderResolver
import com.wasimaster.corvene.platform.rememberFolderPicker
import com.wasimaster.corvene.platform.requestAllFilesAccess

private const val GITHUB_COM = "https://api.github.com"

/**
 * The Clone screen wired to the engine: the GitHub.com account's
 * repositories (`loadCloneableRepositories` / `cloneableRepositories`), the
 * destination (`files/repositories/<name>` by default, or a picked folder),
 * then `cloneRepository(url, path, depth)` and [onClose]; the progress shows
 * in [CloneProgressRoute]. [prefillUrl]: an `openRepo` link or shared text.
 */
@Composable
fun CloneRoute(prefillUrl: String?, onClose: () -> Unit, onSignIn: () -> Unit, modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val context = LocalContext.current
    val session by rememberCoreQuery { session() }
    val account = session.value?.accounts?.firstOrNull { it.endpoint == GITHUB_COM }
    val endpoint = account?.endpoint
    val cloneable by rememberCoreQuery(endpoint) { endpoint?.let { cloneableRepositories(it) } }
    val settings by rememberCoreQuery { settings() }
    // GHD's default clone folder (Settings › Git), else Corvene's own storage
    val defaultBase = settings.value?.cloneDir?.takeIf { it.isNotBlank() }
        ?: remember { CorveneDocumentsProvider.repositoriesDirectory(context).path }
    var chosenBase by rememberSaveable { mutableStateOf<String?>(null) }
    val base = chosenBase ?: defaultBase
    var selectedTab by rememberSaveable { mutableStateOf(if (prefillUrl.isNullOrBlank()) CloneTab.GitHub else CloneTab.Url) }
    var typed by rememberSaveable { mutableStateOf(prefillUrl.orEmpty()) }
    var shallowClone by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    LaunchedEffect(endpoint, selectedTab) {
        if (endpoint != null && selectedTab == CloneTab.GitHub) core.dispatch { loadCloneableRepositories(endpoint) }
    }
    val picker = rememberFolderPicker(import = false) { picked -> if (picked != null) chosenBase = picked }
    val name = repositoryNameOf(cloneUrlFor(typed))
    val path = if (name.isEmpty()) base else "$base/$name"
    val actions = object : CloneActions {
        override fun tab(tab: CloneTab) = run { selectedTab = tab }

        override fun url(url: String) = run { typed = url }

        override fun pick(repository: CloneableRepositoryVm) = run { typed = repository.cloneUrl }

        override fun filter(text: String) = run { query = text }

        override fun choosePath() = picker.pick()

        override fun shallow(on: Boolean) = run { shallowClone = on }

        override fun signIn() = onSignIn()

        override fun clone() {
            val target = cloneUrlFor(typed)
            core.dispatch { cloneRepository(target, path, if (shallowClone) 1u else 0u) }
            onClose()
        }

        override fun close() = onClose()
    }
    CloneScreen(
        CloneState(
            tab = selectedTab,
            url = typed,
            path = path,
            shallow = shallowClone,
            signedIn = account != null,
            loading = cloneable.value?.loading == true,
            repositories = cloneable.value?.repositories,
            filter = query,
        ),
        actions,
        modifier,
    )
}

/** Add Local Repository wired to the engine: the picker (with the import), then `addRepository`. */
@Composable
fun AddRepositoryRoute(onClose: () -> Unit, modifier: Modifier = Modifier, initialPath: String? = null) {
    val core = LocalCore.current
    val context = LocalContext.current
    var path by rememberSaveable { mutableStateOf(initialPath) }
    val picker = rememberFolderPicker { picked -> if (picked != null) path = picked }
    val askAccess = FolderResolver.canRequestAllFilesAccess(context) && !FolderResolver.hasAllFilesAccess(context)
    AddRepositoryScreen(
        path = path,
        onChoose = picker::pick,
        onAdd = {
            val chosen = path ?: return@AddRepositoryScreen
            core.dispatch { addRepository(chosen) }
            onClose()
        },
        onClose = onClose,
        modifier = modifier,
        onAllFilesAccess = if (askAccess) ({ requestAllFilesAccess(context) }) else null,
    )
}

/** Create a New Repository wired to the engine: `createRepository(<base>/<name>, …)`. */
@Composable
fun CreateRepositoryRoute(onClose: () -> Unit, modifier: Modifier = Modifier, initialBase: String? = null) {
    val core = LocalCore.current
    val context = LocalContext.current
    val defaultBase = remember { initialBase ?: CorveneDocumentsProvider.repositoriesDirectory(context).path }
    var base by rememberSaveable { mutableStateOf(defaultBase) }
    var name by rememberSaveable { mutableStateOf("") }
    var description by rememberSaveable { mutableStateOf("") }
    var readme by rememberSaveable { mutableStateOf(true) }
    var gitignore by rememberSaveable { mutableStateOf<String?>(null) }
    var license by rememberSaveable { mutableStateOf<String?>(null) }
    val picker = rememberFolderPicker(import = false) { picked -> if (picked != null) base = picked }
    CreateRepositoryScreen(
        form = CreateForm(name, description, readme, gitignore, license),
        base = base,
        onChange = {
            name = it.name
            description = it.description
            readme = it.readme
            gitignore = it.gitignore
            license = it.license
        },
        onChooseBase = picker::pick,
        onCreate = {
            val trimmed = sanitizedName(name.trim())
            val text = description.trim().takeIf { it.isNotEmpty() }
            core.dispatch { createRepository("$base/$trimmed", trimmed, text, readme, gitignore, license) }
            onClose()
        },
        onClose = onClose,
        modifier = modifier,
    )
}

/**
 * The clone in progress (`session().cloning`), over everything: the step
 * git reports and its progress, Cancel = `cancelClone`. When it ends with
 * the repository added, [onCloned] gets its id.
 */
@Composable
fun CloneProgressRoute(onCloned: (Long) -> Unit) {
    val core = LocalCore.current
    val session by rememberCoreQuery { session() }
    val list by rememberCoreQuery { repoList() }
    val cloning = session.value?.cloning
    var lastPath by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(cloning?.path, list.value) {
        if (cloning != null) {
            lastPath = cloning.path
        } else if (lastPath != null && session.value != null) {
            val added = list.value?.repositories?.firstOrNull { it.path.trimEnd('/') == lastPath?.trimEnd('/') }
            if (added != null) {
                lastPath = null
                onCloned(added.id.toLong())
            }
        }
    }
    if (cloning != null) CloneProgressDialog(cloning, onCancel = { core.dispatch { cancelClone() } })
}

/** GHD's clone progress: the repository, git's current step, a bar (indeterminate without numbers), Cancel. */
@Composable
fun CloneProgressDialog(progress: CloneProgressVm, onCancel: () -> Unit, modifier: Modifier = Modifier) {
    PrimerDialog(
        title = stringResource(R.string.repo_cloning, repositoryNameOf(progress.url)),
        onDismissRequest = {},
        modifier = modifier.testTag(TAG_CLONE_PROGRESS),
        dismissible = false,
        dismissButton = {
            PrimerButton(stringResource(R.string.repo_cancel), onCancel, variant = PrimerButtonVariant.Danger)
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(progress.url, style = CorveneTheme.textStyles.codeSmall, color = CorveneTheme.colors.textSecondary)
            ProgressBar(progress.value)
            Text(progress.description, style = MaterialTheme.typography.bodyMedium, color = CorveneTheme.colors.textPrimary)
        }
    }
}

const val TAG_CLONE_PROGRESS = "repo_clone_progress"
