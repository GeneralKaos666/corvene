package com.wasimaster.corvene.history

import android.content.Context
import android.os.Build
import android.widget.Toast
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.wasimaster.corvene.design.ProgressBar
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.openUrl
import com.wasimaster.corvene.platform.writeClipboard
import kotlinx.coroutines.flow.drop

/**
 * The History tab wired to the engine: `history(repo, 0, 0)` for the count
 * and flags after every state change, a [CommitPager] reading 100-commit
 * windows, the branches for the compare picker. [onOpenCommit] runs after a
 * commit is selected (compact widths push the commit detail);
 * [onCreateBranchFrom] opens the Create Branch form on a commit (the app owns
 * it). [github] is `owner/name` when the repository is on GitHub.
 */
@Composable
fun HistoryRoute(
    repo: Long,
    github: String?,
    onOpenCommit: () -> Unit,
    onCreateBranchFrom: (String) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    val core = LocalCore.current
    val context = LocalContext.current
    val id = repo.toULong()
    val meta by rememberCoreQuery(repo) { history(id, 0u, 0u) }
    val branches by rememberCoreQuery(repo) { branches(id) }
    var compareBranch by rememberSaveable(repo) { mutableStateOf<String?>(null) }
    var compareAhead by rememberSaveable(repo) { mutableStateOf(false) }
    val open by rememberUpdatedState(onOpenCommit)
    val branchFrom by rememberUpdatedState(onCreateBranchFrom)
    val pager = remember(repo, compareBranch, compareAhead) {
        CommitPager(0) { start, count -> core.query { history(id, start.toUInt(), count.toUInt())?.commits.orEmpty() } }
    }
    val total = meta.value?.totalLoaded?.toInt() ?: 0
    LaunchedEffect(pager, total) { pager.resize(total) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(pager, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            core.version.drop(1).collect { pager.refresh() }
        }
    }
    val actions = remember(core, id, github) {
        object : HistoryActions {
            override fun select(sha: String) {
                core.dispatch { selectCommit(id, sha) }
                open()
            }

            override fun selectRange(shas: List<String>) = core.dispatch { selectCommits(id, shas) }

            override fun loadMore() = core.dispatch { loadMoreCommits(id) }

            override fun compare(branch: String?, mode: String) {
                compareBranch = branch
                compareAhead = mode == MODE_AHEAD
                core.dispatch { compareToBranch(id, branch, mode) }
            }

            override fun merge(branch: String) = core.dispatch { mergeBranch(id, branch, false) }

            override fun revert(sha: String) = core.dispatch { revertCommit(id, sha) }

            override fun cherryPick(shas: List<String>) = core.dispatch { startCherryPick(id, shas) }

            override fun createBranchFrom(sha: String) = branchFrom(sha)

            override fun createTag(sha: String, name: String, message: String) = core.dispatch { createTag(id, name, sha, message) }

            override fun checkout(sha: String) = core.dispatch { checkoutCommit(id, sha) }

            override fun reset(sha: String) = core.dispatch { resetToCommit(id, sha) }

            override fun copySha(sha: String) = copy(context, sha)

            override fun viewOnGitHub(sha: String) {
                if (github != null) openUrl(context, "https://github.com/$github/commit/$sha")
            }
        }
    }
    val value = meta.value
    if (value == null) {
        Box(modifier.fillMaxSize()) { if (meta.loading) ProgressBar(null, Modifier.align(Alignment.TopCenter)) }
        return
    }
    val names = branches.value?.branches?.filter { !it.name.endsWith("/HEAD") }?.map { it.name }.orEmpty()
    HistoryScreen(
        history = value,
        pager = pager,
        comparison = compareBranch?.let { Comparison(it, compareAhead) },
        branchNames = names,
        current = branches.value?.current,
        github = github != null,
        actions = actions,
        modifier = modifier,
        contentPadding = contentPadding,
    )
}

/**
 * The selected commit wired to the engine (`commitDetail(repo)`).
 * [onOpenFile] runs after a file is selected (compact widths push its diff).
 */
@Composable
fun CommitDetailRoute(repo: Long, onOpenFile: () -> Unit, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val context = LocalContext.current
    val id = repo.toULong()
    val detail by rememberCoreQuery(repo) { commitDetail(id) }
    val openFile by rememberUpdatedState(onOpenFile)
    val actions = remember(core, id) {
        object : CommitDetailActions {
            override fun selectFile(path: String) {
                core.dispatch { selectCommitFile(id, path) }
                openFile()
            }

            override fun copySha(sha: String) = copy(context, sha)
        }
    }
    val value = detail.value
    if (value == null) {
        Box(modifier.fillMaxSize()) { if (detail.loading) ProgressBar(null, Modifier.align(Alignment.TopCenter)) }
    } else {
        CommitDetailScreen(value, actions, modifier, contentPadding = contentPadding)
    }
}

private fun copy(context: Context, sha: String) {
    writeClipboard(context, sha)
    // Android 13 and later confirm a copy themselves
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
        Toast.makeText(context, context.getString(R.string.hist_copied, sha.take(SHORT)), Toast.LENGTH_SHORT).show()
    }
}

private const val SHORT = 7
