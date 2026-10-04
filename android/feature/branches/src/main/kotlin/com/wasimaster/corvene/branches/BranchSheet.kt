package com.wasimaster.corvene.branches

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.common.relativeTime
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuDivider
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.design.StateLabel
import com.wasimaster.corvene.design.StateLabelState
import com.wasimaster.corvene.design.UnderlineNav
import com.wasimaster.corvene.design.UnderlineNavItem
import com.wasimaster.corvene.ffi.gen.BranchVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.PullRequestVm
import com.wasimaster.corvene.ffi.gen.PullRequestsVm

/** What the branch panel sends back. */
interface BranchActions {
    /** Tap on a branch: the engine asks about uncommitted changes when there are any. */
    fun checkout(name: String)

    fun create(name: String, startPoint: String?)

    fun rename(old: String, new: String)

    fun delete(name: String, includeRemote: Boolean)

    /** Merge [name] into the current branch (also "Update from <default>"). */
    fun merge(name: String)

    /** Rebase the current branch onto [name] (`856-branch-menu-rebase-onto`). */
    fun rebaseOnto(name: String)

    /** History › Compare to [name]. */
    fun compare(name: String)

    fun checkoutPullRequest(number: ULong)
}

/**
 * GHD's branch foldout as a [SelectPanel]: Branches | Pull Requests tabs
 * (the second only for GitHub repositories, [pullRequests] non-null), the
 * filter, "New branch", the groups Default / Recent / Other and the remote
 * branches folded under "Remote"; the current branch checked, each tip's
 * relative time at the end. A tap checks the branch out and closes the
 * panel; a long press opens the branch's menu (Rename, Delete, Merge into
 * current, Rebase onto, Compare, Update from default). Create, rename and
 * delete ask in their own dialogs first.
 */
@Composable
fun BranchSheet(
    branches: BranchesVm,
    pullRequests: PullRequestsVm?,
    actions: BranchActions,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    inline: Boolean = false,
    now: Long = System.currentTimeMillis(),
) {
    var filter by rememberSaveable { mutableStateOf("") }
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var remoteOpen by rememberSaveable { mutableStateOf(false) }
    var menuFor by remember { mutableStateOf<String?>(null) }
    var creating by rememberSaveable { mutableStateOf<String?>(null) }
    var renaming by rememberSaveable { mutableStateOf<String?>(null) }
    var deleting by rememberSaveable { mutableStateOf<String?>(null) }
    val groups = remember(branches, filter) { groupBranches(branches, filter) }
    val showPulls = pullRequests != null && tab == 1
    val rows = BranchRows(
        branches = branches,
        now = now,
        menuFor = menuFor,
        onMenu = { menuFor = it },
        onPick = { branch ->
            if (!branch.current) actions.checkout(branch.name)
            onDismissRequest()
        },
        menu = BranchMenuActions(
            rename = { renaming = it },
            delete = { deleting = it },
            merge = {
                actions.merge(it)
                onDismissRequest()
            },
            rebase = {
                actions.rebaseOnto(it)
                onDismissRequest()
            },
            compare = {
                actions.compare(it)
                onDismissRequest()
            },
        ),
    )
    val labels = GroupLabels(
        default = stringResource(R.string.br_group_default),
        recent = stringResource(R.string.br_group_recent),
        other = stringResource(R.string.br_group_other),
        remote = stringResource(if (remoteOpen || filter.isNotBlank()) R.string.br_group_remote_open else R.string.br_group_remote),
    )
    SelectPanel(
        title = stringResource(R.string.br_title),
        onDismissRequest = onDismissRequest,
        modifier = modifier.testTag(TAG_SHEET),
        filter = filter,
        onFilterChange = if (showPulls) null else ({ filter = it }),
        filterPlaceholder = stringResource(R.string.br_filter),
        closeDescription = stringResource(R.string.br_close),
        inline = inline,
        titleActions = {
            if (!showPulls) {
                PrimerButton(
                    stringResource(R.string.br_new),
                    { creating = filter },
                    leadingIcon = Octicons.Plus,
                    modifier = Modifier.testTag(TAG_NEW),
                )
            }
        },
        tabs = pullRequests?.let { prs ->
            {
                UnderlineNav(
                    listOf(
                        UnderlineNavItem(stringResource(R.string.br_tab_branches)),
                        UnderlineNavItem(stringResource(R.string.br_tab_pulls), prs.pullRequests.size),
                    ),
                    selectedIndex = tab,
                    onSelect = { tab = it },
                )
            }
        },
    ) {
        if (showPulls) {
            pullRequestItems(pullRequests.pullRequests, actions, onDismissRequest)
        } else {
            group("default", labels.default, groups.default, rows)
            group("recent", labels.recent, groups.recent, rows)
            group("other", labels.other, groups.other, rows)
            if (groups.remote.isNotEmpty()) {
                val open = remoteOpen || filter.isNotBlank()
                item(key = "h:remote", contentType = "toggle") {
                    ActionListItem(
                        labels.remote,
                        onClick = { remoteOpen = !remoteOpen },
                        count = groups.remote.size,
                        modifier = Modifier.testTag(TAG_REMOTE),
                        leading = { Octicon(
                            if (open) Octicons.ChevronDown else Octicons.ChevronRight,
                            null,
                            tint = OcticonTint.Secondary,
                        ) },
                    )
                }
                if (open) branchItems("remote", groups.remote, rows)
            }
            if (groups.isEmpty) {
                item(key = "empty") { ActionListItem(stringResource(R.string.br_no_match, filter), enabled = false) }
            }
        }
        item(key = "pad") { Spacer(Modifier.height(CorveneTheme.spacing.s)) }
    }
    creating?.let { initial ->
        CreateBranchDialog(
            initialName = initial,
            current = branches.current,
            defaultBranch = branches.defaultBranch,
            targetSha = null,
            existing = branches.branches.map { it.name }.toSet(),
            onCreate = { name, start ->
                creating = null
                actions.create(name, start)
                onDismissRequest()
            },
            onDismissRequest = { creating = null },
        )
    }
    renaming?.let { name ->
        RenameBranchDialog(
            name = name,
            existing = branches.branches.map { it.name }.toSet(),
            onRename = { new ->
                renaming = null
                actions.rename(name, new)
            },
            onDismissRequest = { renaming = null },
        )
    }
    deleting?.let { name ->
        DeleteBranchDialog(
            name = name,
            hasRemote = branches.branches.firstOrNull { it.name == name }?.upstream != null,
            onDelete = { remote ->
                deleting = null
                actions.delete(name, remote)
            },
            onDismissRequest = { deleting = null },
        )
    }
}

private class GroupLabels(val default: String, val recent: String, val other: String, val remote: String)

private class BranchMenuActions(
    val rename: (String) -> Unit,
    val delete: (String) -> Unit,
    val merge: (String) -> Unit,
    val rebase: (String) -> Unit,
    val compare: (String) -> Unit,
)

/** What every branch row needs. */
private class BranchRows(
    val branches: BranchesVm,
    val now: Long,
    val menuFor: String?,
    val onMenu: (String?) -> Unit,
    val onPick: (BranchVm) -> Unit,
    val menu: BranchMenuActions,
)

private fun LazyListScope.group(key: String, title: String, branches: List<BranchVm>, rows: BranchRows) {
    if (branches.isEmpty()) return
    item(key = "h:$key", contentType = "header") { ActionListGroupHeader(title) }
    branchItems(key, branches, rows)
}

private fun LazyListScope.branchItems(key: String, branches: List<BranchVm>, rows: BranchRows) {
    items(branches, key = { "$key:${it.name}" }, contentType = { "branch" }) { branch -> BranchRow(branch, rows) }
}

@Composable
private fun BranchRow(branch: BranchVm, rows: BranchRows) {
    Box {
        ActionListItem(
            branch.name,
            checked = branch.current,
            onClick = { rows.onPick(branch) },
            onLongClick = { rows.onMenu(branch.name) },
            modifier = Modifier.testTag("$TAG_BRANCH${branch.name}"),
            leading = { Octicon(Octicons.GitBranch, null, tint = OcticonTint.Secondary) },
            trailing = branch.tipTime?.let { time ->
                {
                    Text(
                        relativeTime(time, rows.now),
                        style = MaterialTheme.typography.bodySmall,
                        color = CorveneTheme.colors.textSecondary,
                    )
                }
            },
        )
        BranchMenu(branch, rows, expanded = rows.menuFor == branch.name)
    }
}

/** A branch's long-press menu (GHD's branch context menu plus the Branch menu's merge/rebase/compare). */
@Composable
private fun BranchMenu(branch: BranchVm, rows: BranchRows, expanded: Boolean) {
    val vm = rows.branches
    val current = vm.current
    val close = { rows.onMenu(null) }
    fun run(action: (String) -> Unit): () -> Unit = {
        close()
        action(branch.name)
    }
    ActionMenu(expanded = expanded, onDismissRequest = close) {
        if (!branch.remote) {
            ActionMenuItem(stringResource(R.string.br_menu_rename), run(rows.menu.rename), leadingIcon = Octicons.Pencil)
        }
        if (current != null && !branch.current) {
            ActionMenuItem(
                stringResource(R.string.br_menu_merge, branch.name, current),
                run(rows.menu.merge),
                leadingIcon = Octicons.GitMerge,
                modifier = Modifier.testTag(TAG_MENU_MERGE),
            )
            ActionMenuItem(
                stringResource(R.string.br_menu_rebase, current, branch.name),
                run(rows.menu.rebase),
                leadingIcon = Octicons.GitCommit,
            )
        }
        val default = vm.defaultBranch
        if (branch.current && default != null && default != branch.name) {
            ActionMenuItem(
                stringResource(R.string.br_menu_update, default),
                {
                    close()
                    rows.menu.merge(default)
                },
                leadingIcon = Octicons.ArrowDown,
            )
        }
        if (!branch.current) {
            ActionMenuItem(stringResource(R.string.br_menu_compare), run(rows.menu.compare), leadingIcon = Octicons.GitCompare)
        }
        if (!branch.remote) {
            ActionMenuDivider()
            ActionMenuItem(
                stringResource(R.string.br_menu_delete),
                run(rows.menu.delete),
                leadingIcon = Octicons.Trash,
                danger = true,
                enabled = !branch.current,
            )
        }
    }
}

private fun LazyListScope.pullRequestItems(pulls: List<PullRequestVm>, actions: BranchActions, onDismiss: () -> Unit) {
    if (pulls.isEmpty()) {
        item(key = "pr:empty") {
            Column {
                ActionListItem(stringResource(R.string.br_pulls_empty), enabled = false)
            }
        }
        return
    }
    items(pulls, key = { "pr:${it.number}" }, contentType = { "pull" }) { pr ->
        ActionListItem(
            pr.title,
            description = stringResource(R.string.br_pull_meta, pr.number.toLong(), pr.author, pr.headRef),
            checked = if (pr.checkedOut) true else null,
            onClick = {
                actions.checkoutPullRequest(pr.number)
                onDismiss()
            },
            modifier = Modifier.testTag("$TAG_PULL${pr.number}"),
            leading = {
                Octicon(
                    if (pr.draft) Octicons.GitPullRequestDraft else Octicons.GitPullRequest,
                    null,
                    tint = if (pr.draft) OcticonTint.Secondary else OcticonTint.Success,
                )
            },
            trailing = {
                StateLabel(
                    stringResource(if (pr.draft) R.string.br_pull_draft else R.string.br_pull_open),
                    if (pr.draft) StateLabelState.Draft else StateLabelState.Open,
                )
                if (!pr.checkedOut) {
                    PrimerButton(
                        stringResource(R.string.br_pull_checkout),
                        {
                            actions.checkoutPullRequest(pr.number)
                            onDismiss()
                        },
                        variant = PrimerButtonVariant.Default,
                    )
                }
            },
        )
    }
}

const val TAG_SHEET = "br_sheet"
const val TAG_NEW = "br_new"
const val TAG_REMOTE = "br_remote"
const val TAG_BRANCH = "br_branch_"
const val TAG_PULL = "br_pull_"
const val TAG_MENU_MERGE = "br_menu_merge"
