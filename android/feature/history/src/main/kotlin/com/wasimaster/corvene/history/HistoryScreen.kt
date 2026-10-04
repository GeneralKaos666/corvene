package com.wasimaster.corvene.history

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.common.relativeTime
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuDivider
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.Avatar
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.ChipRow
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Label
import com.wasimaster.corvene.design.LabelVariant
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerChip
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.design.SegmentedControl
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.ffi.gen.CommitVm
import com.wasimaster.corvene.ffi.gen.HistoryVm
import kotlinx.coroutines.flow.collectLatest

/** What the History list sends back. */
interface HistoryActions {
    fun select(sha: String)

    /** A contiguous range picked in multi-select mode (`selectCommits`). */
    fun selectRange(shas: List<String>)

    /** The list neared its end (`loadMoreCommits`). */
    fun loadMore()

    /** Compare to [branch] ("behind" / "ahead"); null ends the comparison. */
    fun compare(branch: String?, mode: String)

    /** Merge the compared branch into the current one. */
    fun merge(branch: String)

    fun revert(sha: String)

    fun cherryPick(shas: List<String>)

    fun createBranchFrom(sha: String)

    fun createTag(sha: String, name: String, message: String)

    fun checkout(sha: String)

    fun reset(sha: String)

    fun copySha(sha: String)

    fun viewOnGitHub(sha: String)
}

/** What History compares against: GHD's compare branch and its Ahead / Behind tab. */
@Immutable
data class Comparison(val branch: String, val ahead: Boolean)

/**
 * The History tab (GHD's CompareSidebar): the "Compare to branch" chip (a
 * [SelectPanel] of [branchNames]) or, while comparing, the branch with the
 * Behind | Ahead [SegmentedControl] and a merge button; then the commits,
 * paged from the engine by [pager] ([history] carries the count and whether
 * more can load; the list asks [HistoryActions.loadMore] near its end).
 * Rows: avatar, summary, author · relative time, tags, a merge icon. A tap
 * selects the commit; a long press opens its menu, whose "Select multiple"
 * turns taps into contiguous range selection until Done.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HistoryScreen(
    history: HistoryVm,
    pager: CommitPager,
    comparison: Comparison?,
    branchNames: List<String>,
    current: String?,
    github: Boolean,
    actions: HistoryActions,
    modifier: Modifier = Modifier,
    now: Long = System.currentTimeMillis(),
    contentPadding: PaddingValues = PaddingValues(),
) {
    var anchor by rememberSaveable { mutableStateOf<Int?>(null) }
    var menuFor by remember { mutableStateOf<Int?>(null) }
    var tagging by rememberSaveable { mutableStateOf<String?>(null) }
    var checkingOut by rememberSaveable { mutableStateOf<String?>(null) }
    var resetting by rememberSaveable { mutableStateOf<String?>(null) }
    var picking by rememberSaveable { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val latest by rememberUpdatedState(history)
    val loadMore by rememberUpdatedState(actions::loadMore)
    LaunchedEffect(pager, listState) {
        var askedAt = -1
        snapshotFlow { listState.firstVisibleItemIndex to (listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) }
            .collectLatest { (first, last) ->
                val total = latest.totalLoaded.toInt()
                if (last >= total - LOAD_AHEAD && !latest.exhausted && !latest.loading && askedAt != total) {
                    askedAt = total
                    loadMore()
                }
                pager.ensure(first, maxOf(first, last))
            }
    }
    val menu = CommitMenuActions(
        actions = actions,
        github = github,
        onSelectMultiple = { anchor = it },
        onTag = { tagging = it },
        onCheckout = { checkingOut = it },
        onReset = { resetting = it },
    )
    Column(modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas)) {
        CompareBar(comparison, current, history, actions, onPick = { picking = true })
        val selecting = anchor
        if (selecting != null) {
            SelectionBar(
                count = history.selected.size,
                onCherryPick = { actions.cherryPick(history.selected) },
                onDone = { anchor = null },
            )
        }
        if (history.loading && history.totalLoaded == 0u) ProgressBar(null)
        if (history.totalLoaded == 0u && !history.loading) {
            Blankslate(
                Octicons.History,
                stringResource(if (comparison != null) R.string.hist_compare_empty else R.string.hist_empty),
                Modifier.fillMaxSize().testTag(TAG_EMPTY),
            )
        } else {
            LazyColumn(Modifier.weight(1f).fillMaxWidth().testTag(TAG_LIST), state = listState, contentPadding = contentPadding) {
                items(pager.total, key = { it }, contentType = { "commit" }) { index ->
                    val commit = pager.commit(index)
                    if (commit == null) {
                        Box(Modifier.fillMaxWidth().height(CorveneTheme.metrics.rowHeightLarge))
                    } else {
                        Box {
                            CommitRow(
                                commit,
                                now,
                                onClick = {
                                    val from = anchor
                                    if (from == null) actions.select(commit.sha) else actions.selectRange(pager.shas(from, index))
                                },
                                onLongClick = { if (anchor == null) menuFor = index else actions.selectRange(pager.shas(
                                    anchor ?: index,
                                    index,
                                )) },
                            )
                            CommitMenu(commit, index, menu, expanded = menuFor == index, onDismiss = { menuFor = null })
                        }
                    }
                }
            }
        }
    }
    if (picking) {
        var filter by rememberSaveable { mutableStateOf("") }
        val shown = branchNames.filter { it != current && it.contains(filter.trim(), ignoreCase = true) }
        SelectPanel(
            title = stringResource(R.string.hist_compare_title),
            onDismissRequest = { picking = false },
            filter = filter,
            onFilterChange = { filter = it },
            filterPlaceholder = stringResource(R.string.hist_filter),
            closeDescription = stringResource(R.string.hist_close),
        ) {
            item(key = "h") { ActionListGroupHeader(stringResource(R.string.hist_compare_branches)) }
            items(shown, key = { it }) { name ->
                ActionListItem(
                    name,
                    onClick = {
                        picking = false
                        actions.compare(name, MODE_BEHIND)
                    },
                    modifier = Modifier.testTag("$TAG_COMPARE_BRANCH$name"),
                    leading = { Octicon(Octicons.GitBranch, null, tint = OcticonTint.Secondary) },
                )
            }
        }
    }
    tagging?.let { sha ->
        CreateTagDialog(
            sha,
            onCreate = { name, message ->
                tagging = null
                actions.createTag(sha, name, message)
            },
            onDismissRequest = { tagging = null },
        )
    }
    checkingOut?.let { sha ->
        ConfirmCheckoutCommitDialog(
            onCheckout = {
                checkingOut = null
                actions.checkout(sha)
            },
            onDismissRequest = { checkingOut = null },
        )
    }
    resetting?.let { sha ->
        WarningBeforeResetDialog(
            onReset = {
                resetting = null
                actions.reset(sha)
            },
            onDismissRequest = { resetting = null },
        )
    }
}

@Composable
private fun CompareBar(comparison: Comparison?, current: String?, history: HistoryVm, actions: HistoryActions, onPick: () -> Unit) {
    val colors = CorveneTheme.colors
    if (comparison == null) {
        ChipRow {
            PrimerChip(
                stringResource(R.string.hist_compare),
                selected = false,
                onClick = onPick,
                leadingIcon = Octicons.GitCompare,
                dropdown = true,
                modifier = Modifier.testTag(TAG_COMPARE),
            )
        }
        HorizontalDivider(color = colors.borderMuted)
        return
    }
    Column(
        Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Octicon(Octicons.GitCompare, null, tint = OcticonTint.Secondary)
            Text(
                stringResource(R.string.hist_comparing, comparison.branch),
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.textPrimary,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
            )
            PrimerIconButton(
                Octicons.X,
                stringResource(R.string.hist_compare_end),
                { actions.compare(null, MODE_BEHIND) },
                tint = OcticonTint.Secondary,
                modifier = Modifier.testTag(TAG_COMPARE_END),
            )
        }
        SegmentedControl(
            listOf(stringResource(R.string.hist_behind), stringResource(R.string.hist_ahead)),
            selectedIndex = if (comparison.ahead) 1 else 0,
            onSelect = { actions.compare(comparison.branch, if (it == 1) MODE_AHEAD else MODE_BEHIND) },
            modifier = Modifier.fillMaxWidth().testTag(TAG_COMPARE_MODE),
            fill = true,
        )
        val count = history.totalLoaded.toInt()
        Text(
            pluralStringResource(
                if (comparison.ahead) R.plurals.hist_ahead_count else R.plurals.hist_behind_count,
                count,
                count,
                comparison.branch,
            ),
            style = MaterialTheme.typography.bodySmall,
            color = colors.textSecondary,
        )
        if (!comparison.ahead && current != null) {
            PrimerButton(
                stringResource(R.string.hist_merge_into, comparison.branch, current),
                { actions.merge(comparison.branch) },
                variant = PrimerButtonVariant.Primary,
                enabled = count > 0,
                leadingIcon = Octicons.GitMerge,
                modifier = Modifier.fillMaxWidth().testTag(TAG_MERGE),
            )
        }
    }
    HorizontalDivider(color = colors.borderMuted)
}

@Composable
private fun SelectionBar(count: Int, onCherryPick: () -> Unit, onDone: () -> Unit) {
    val colors = CorveneTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .background(colors.accent.subtle)
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 4.dp)
            .testTag(TAG_SELECTION),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            pluralStringResource(R.plurals.hist_selected, count, count),
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
            color = colors.textPrimary,
        )
        PrimerButton(stringResource(R.string.hist_cherry_pick_short), onCherryPick, enabled = count > 0)
        PrimerButton(stringResource(R.string.hist_done), onDone, variant = PrimerButtonVariant.Link)
    }
}

/** One commit: avatar, summary, author · relative time, tags; selected rows highlighted. */
@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
internal fun CommitRow(commit: CommitVm, now: Long, onClick: () -> Unit, onLongClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .semantics { selected = commit.selected }
            .background(if (commit.selected) colors.bgSelected else colors.bgCanvas)
            .combinedClickable(role = Role.Button, onLongClick = onLongClick, onClick = onClick)
            .heightIn(min = CorveneTheme.metrics.rowHeightLarge)
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp)
            .testTag("$TAG_COMMIT${commit.sha}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Avatar(commit.authorName, size = 32.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                commit.summary,
                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                color = colors.textPrimary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                stringResource(R.string.hist_byline, commit.authorName, relativeTime(commit.authoredAt, now)),
                style = MaterialTheme.typography.bodySmall,
                color = colors.textSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (commit.tags.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    commit.tags.forEach { Label(it, variant = LabelVariant.Accent) }
                }
            }
        }
        if (commit.isMerge) Octicon(Octicons.GitMerge, stringResource(R.string.hist_merge_commit), tint = OcticonTint.Secondary)
    }
}

private class CommitMenuActions(
    val actions: HistoryActions,
    val github: Boolean,
    val onSelectMultiple: (Int) -> Unit,
    val onTag: (String) -> Unit,
    val onCheckout: (String) -> Unit,
    val onReset: (String) -> Unit,
)

/** GHD's commit context menu (`CommitListItem.getContextMenuItems`), plus "Select multiple". */
@Composable
private fun CommitMenu(commit: CommitVm, index: Int, menu: CommitMenuActions, expanded: Boolean, onDismiss: () -> Unit) {
    fun run(action: () -> Unit): () -> Unit = {
        onDismiss()
        action()
    }
    val actions = menu.actions
    ActionMenu(expanded = expanded, onDismissRequest = onDismiss) {
        ActionMenuItem(stringResource(R.string.hist_menu_select), run { menu.onSelectMultiple(index) }, leadingIcon = Octicons.Check)
        ActionMenuDivider()
        ActionMenuItem(
            stringResource(R.string.hist_menu_revert),
            run { actions.revert(commit.sha) },
            leadingIcon = Octicons.Undo,
            modifier = Modifier.testTag(TAG_MENU_REVERT),
        )
        ActionMenuItem(
            stringResource(R.string.hist_menu_cherry_pick),
            run { actions.cherryPick(listOf(commit.sha)) },
            leadingIcon = Octicons.GitCommit,
        )
        ActionMenuItem(
            stringResource(R.string.hist_menu_branch),
            run { actions.createBranchFrom(commit.sha) },
            leadingIcon = Octicons.GitBranch,
        )
        ActionMenuItem(
            stringResource(R.string.hist_menu_tag),
            run { menu.onTag(commit.sha) },
            leadingIcon = Octicons.Tag,
            modifier = Modifier.testTag(TAG_MENU_TAG),
        )
        ActionMenuItem(stringResource(R.string.hist_menu_checkout), run { menu.onCheckout(commit.sha) }, leadingIcon = Octicons.Eye)
        ActionMenuItem(stringResource(R.string.hist_menu_reset), run { menu.onReset(commit.sha) }, leadingIcon = Octicons.History)
        ActionMenuDivider()
        ActionMenuItem(
            stringResource(R.string.hist_menu_copy_sha),
            run { actions.copySha(commit.sha) },
            leadingIcon = Octicons.Copy,
            modifier = Modifier.testTag(TAG_MENU_COPY),
        )
        ActionMenuItem(
            stringResource(R.string.hist_menu_github),
            run { actions.viewOnGitHub(commit.sha) },
            leadingIcon = Octicons.LinkExternal,
            enabled = menu.github,
        )
    }
}

/** `compareToBranch` modes. */
const val MODE_BEHIND = "behind"
const val MODE_AHEAD = "ahead"

/** Ask for more this many rows before the end. */
private const val LOAD_AHEAD = 20

const val TAG_LIST = "hist_list"
const val TAG_EMPTY = "hist_empty"
const val TAG_COMMIT = "hist_commit_"
const val TAG_COMPARE = "hist_compare"
const val TAG_COMPARE_BRANCH = "hist_compare_branch_"
const val TAG_COMPARE_END = "hist_compare_end"
const val TAG_COMPARE_MODE = "hist_compare_mode"
const val TAG_MERGE = "hist_merge"
const val TAG_SELECTION = "hist_selection"
const val TAG_MENU_REVERT = "hist_menu_revert"
const val TAG_MENU_TAG = "hist_menu_tag"
const val TAG_MENU_COPY = "hist_menu_copy"
