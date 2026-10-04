package com.wasimaster.corvene.repositories

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.FilterField
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.Spinner
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.design.UnderlineNav
import com.wasimaster.corvene.design.UnderlineNavItem
import com.wasimaster.corvene.ffi.gen.CloneableRepositoryVm

/** The Clone dialog's tabs (GHD: GitHub.com, GitHub Enterprise, URL; Enterprise comes with its accounts). */
enum class CloneTab { GitHub, Url }

/** What the Clone screen shows. [repositories] null = not loaded (or not signed in). */
@Immutable
data class CloneState(
    val tab: CloneTab,
    val url: String,
    val path: String,
    val shallow: Boolean,
    val signedIn: Boolean,
    val loading: Boolean = false,
    val repositories: List<CloneableRepositoryVm>? = null,
    val filter: String = "",
)

/** What the Clone screen's controls do. */
interface CloneActions {
    fun tab(tab: CloneTab)

    fun url(url: String)

    fun pick(repository: CloneableRepositoryVm)

    fun filter(text: String)

    fun choosePath()

    fun shallow(on: Boolean)

    fun signIn()

    fun clone()

    fun close()
}

/**
 * GHD's Clone a Repository, full screen: the GitHub.com tab lists the
 * account's repositories (filterable; a pick fills the URL), or asks to sign
 * in; the URL tab takes a URL or `owner/name`. Under both: the local path
 * (Choose… picks another folder) and the shallow clone switch. Clone is the
 * top bar's action.
 */
@Composable
fun CloneScreen(state: CloneState, actions: CloneActions, modifier: Modifier = Modifier) {
    RepositoryForm(
        title = stringResource(R.string.repo_clone_title),
        confirm = stringResource(R.string.repo_clone),
        confirmEnabled = state.url.isNotBlank() && state.path.isNotBlank(),
        onConfirm = actions::clone,
        onClose = actions::close,
        modifier = modifier,
    ) {
        UnderlineNav(
            listOf(UnderlineNavItem(stringResource(R.string.repo_tab_github)), UnderlineNavItem(stringResource(R.string.repo_tab_url))),
            selectedIndex = state.tab.ordinal,
            onSelect = { actions.tab(CloneTab.entries[it]) },
        )
        when (state.tab) {
            CloneTab.GitHub -> GitHubTab(state, actions)
            CloneTab.Url -> {
                FormFields {
                    PrimerTextField(
                        state.url,
                        actions::url,
                        Modifier.testTag(TAG_CLONE_URL),
                        label = stringResource(R.string.repo_clone_url),
                        placeholder = stringResource(R.string.repo_clone_url_hint),
                    )
                }
                Destination(state, actions)
            }
        }
    }
}

@Composable
private fun ColumnScope.GitHubTab(state: CloneState, actions: CloneActions) {
    if (!state.signedIn) {
        Blankslate(
            icon = Octicons.MarkGithub,
            title = stringResource(R.string.repo_clone_sign_in_title),
            description = stringResource(R.string.repo_clone_sign_in_body),
            primaryAction = {
                PrimerButton(
                    stringResource(R.string.repo_sign_in),
                    actions::signIn,
                    Modifier.testTag(TAG_CLONE_SIGN_IN),
                    variant = PrimerButtonVariant.Primary,
                )
            },
            secondaryAction = {
                PrimerButton(stringResource(R.string.repo_use_url), { actions.tab(CloneTab.Url) }, variant = PrimerButtonVariant.Link)
            },
        )
        return
    }
    FilterField(
        state.filter,
        actions::filter,
        Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
        placeholder = stringResource(R.string.repo_filter),
    )
    val visible = state.repositories.orEmpty().filter {
        state.filter.isBlank() || "${it.owner}/${it.name}".contains(state.filter.trim(), ignoreCase = true)
    }
    Box(Modifier.weight(1f).fillMaxWidth()) {
        when {
            state.repositories == null || (state.loading && visible.isEmpty()) ->
                Spinner(Modifier.align(Alignment.Center), label = stringResource(R.string.repo_loading))
            visible.isEmpty() -> Blankslate(Octicons.Repo, stringResource(R.string.repo_no_match))
            else -> LazyColumn(Modifier.testTag(TAG_CLONE_LIST)) {
                items(visible, key = { it.cloneUrl }) { repo ->
                    ActionListItem(
                        title = "${repo.owner}/${repo.name}",
                        selected = state.url == repo.cloneUrl,
                        checked = state.url == repo.cloneUrl,
                        onClick = { actions.pick(repo) },
                        leading = {
                            val colors = CorveneTheme.colors
                            when {
                                repo.private -> IconTile(Octicons.Lock, colors.attention)
                                repo.fork -> IconTile(Octicons.RepoForked, colors.accent)
                                else -> IconTile(Octicons.Repo, colors.accent)
                            }
                        },
                    )
                    ActionListDivider()
                }
            }
        }
    }
    Destination(state, actions)
}

@Composable
private fun Destination(state: CloneState, actions: CloneActions) {
    Column {
        LocalPathRow(state.path, actions::choosePath)
        SwitchRow(
            stringResource(R.string.repo_shallow),
            state.shallow,
            actions::shallow,
            Modifier.testTag(TAG_CLONE_SHALLOW),
            caption = stringResource(R.string.repo_shallow_caption),
        )
    }
}

const val TAG_CLONE_URL = "repo_clone_url"
const val TAG_CLONE_LIST = "repo_clone_list"
const val TAG_CLONE_SHALLOW = "repo_clone_shallow"
const val TAG_CLONE_SIGN_IN = "repo_clone_sign_in"
