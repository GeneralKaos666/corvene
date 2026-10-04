package com.wasimaster.corvene.repositories

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.ffi.gen.RepoListVm
import com.wasimaster.corvene.ffi.gen.RepoVm

/**
 * GHD's Repository foldout as a [SelectPanel] (the title's switcher in the
 * repository chrome): the engine's groups, filterable, the selected one
 * checked; a tap selects and closes. The footer adds a repository.
 */
@Composable
fun RepositoryPicker(
    list: RepoListVm,
    onSelect: (RepoVm) -> Unit,
    onAdd: () -> Unit,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var filter by rememberSaveable { mutableStateOf("") }
    val groups = remember(list, filter) { groupRepositories(list, filter) }
    val recent = stringResource(R.string.repo_group_recent)
    val other = stringResource(R.string.repo_group_other)
    SelectPanel(
        title = stringResource(R.string.repo_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        filter = filter,
        onFilterChange = { filter = it },
        filterPlaceholder = stringResource(R.string.repo_filter),
        closeDescription = stringResource(R.string.repo_close),
        footer = {
            PrimerButton(stringResource(R.string.repo_add), onAdd, leadingIcon = Octicons.Plus)
        },
    ) {
        groups.forEach { group ->
            item(key = "h:${group.kind}:${group.title}", contentType = "header") {
                ActionListGroupHeader(
                    when (group.kind) {
                        RepositoryGroup.Kind.Recent -> recent
                        RepositoryGroup.Kind.Owner -> group.title
                        RepositoryGroup.Kind.Other -> other
                    },
                )
            }
            items(group.repositories, key = { "${group.kind}:${group.title}:${it.id}" }, contentType = { "row" }) { repo ->
                ActionListItem(
                    repo.name,
                    checked = repo.id == list.selected,
                    onClick = { onSelect(repo) },
                    leading = {
                        Octicon(
                            when {
                                repo.missing -> Octicons.Alert
                                repo.private -> Octicons.Lock
                                repo.fork -> Octicons.RepoForked
                                repo.github != null -> Octicons.Repo
                                else -> Octicons.DeviceDesktop
                            },
                            null,
                            tint = if (repo.missing) OcticonTint.Attention else OcticonTint.Secondary,
                        )
                    },
                    trailing = if (repo.changedFiles > 0u) {
                        { Octicon(Octicons.DotFill, stringResource(R.string.repo_changed_files), tint = OcticonTint.Link) }
                    } else {
                        null
                    },
                )
            }
        }
        if (groups.isEmpty()) {
            item(key = "empty") {
                ActionListItem(stringResource(R.string.repo_no_match), enabled = false)
            }
        }
        item(key = "pad") { Spacer(Modifier.height(CorveneTheme.spacing.s)) }
    }
}
