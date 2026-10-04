package com.wasimaster.corvene

import android.app.Application
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import com.wasimaster.corvene.branches.TAG_AUTH_CONFIRM
import com.wasimaster.corvene.branches.TAG_CREATE_CONFIRM
import com.wasimaster.corvene.branches.TAG_DELETE_CONFIRM
import com.wasimaster.corvene.branches.TAG_FORCE_CONFIRM
import com.wasimaster.corvene.branches.TAG_NAME
import com.wasimaster.corvene.branches.TAG_PUBLISH_CONFIRM
import com.wasimaster.corvene.branches.TAG_RENAME_CONFIRM
import com.wasimaster.corvene.branches.TAG_SWITCH_CONFIRM
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.BranchVm
import com.wasimaster.corvene.ffi.gen.BranchesVm
import com.wasimaster.corvene.ffi.gen.KeyList
import com.wasimaster.corvene.ffi.gen.KeyValue
import com.wasimaster.corvene.ffi.gen.PopupVm
import com.wasimaster.corvene.ffi.gen.SyncActionVm
import com.wasimaster.corvene.history.TAG_CHECKOUT_CONFIRM
import com.wasimaster.corvene.history.TAG_RESET_CONFIRM
import com.wasimaster.corvene.history.TAG_TAG_CONFIRM
import com.wasimaster.corvene.history.TAG_TAG_NAME
import com.wasimaster.corvene.mco.TAG_CHOOSE_BRANCH
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Every popup kind the branch, history, sync and multi-commit flows produce:
 * its dialog draws, its primary action dispatches what GHD's does.
 */
@RunWith(ParameterizedRobolectricTestRunner::class)
@Config(application = Application::class)
class PopupDialogTest(private val case: Case) {

    @get:Rule
    val compose = createComposeRule()

    /** One popup: what the engine sends, what the test does, what must be dispatched. */
    class Case(
        val kind: String,
        val fields: Map<String, String> = emptyMap(),
        val lists: Map<String, List<String>> = emptyMap(),
        val expected: String,
        val act: ComposeContentTestRule.() -> Unit = { primary().performClick() },
    ) {
        override fun toString() = kind
    }

    @Test
    fun `the primary action dispatches`() {
        val sent = mutableListOf<String>()
        val popup = PopupVm(
            kind = case.kind,
            repo = if (case.kind in NO_REPO) null else 1u,
            fields = case.fields.map { (k, v) -> KeyValue(k, v) },
            lists = case.lists.map { (k, v) -> KeyList(k, v) },
        )
        compose.setContent {
            CorveneTheme(DesignStyle.GitHubMobile, ColorMode.Light, highContrast = false, dynamicColor = false) {
                PopupDialog(popup, Context, recorder(sent), multiCommitOperation = { sent += "mco $it" })
            }
        }
        compose.(case.act)()
        compose.waitForIdle()
        assertEquals(listOf(case.expected), sent)
    }

    companion object {
        private val NO_REPO = setOf("Error", "ExternalEditorError", "ShellError", "GenericGitAuthentication", "Foo")

        private fun ComposeContentTestRule.primary(): SemanticsNodeInteraction = onNodeWithTag(TAG_POPUP_PRIMARY)

        private fun ComposeContentTestRule.tag(tag: String): SemanticsNodeInteraction = onNodeWithTag(tag)

        private fun ComposeContentTestRule.typeInto(fieldTag: String, text: String) =
            onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(fieldTag))).performTextReplacement(text)

        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}")
        fun cases(): List<Array<Any>> = listOf(
            Case("Error", mapOf("title" to "Could not commit", "text" to "Author identity unknown"), expected = "close"),
            Case(
                "DiscardChanges",
                mapOf("all" to "false"),
                mapOf("paths" to listOf("a.txt", "b.txt")),
                expected = "discard 1 [a.txt, b.txt]",
            ),
            Case(
                "DeleteBranch",
                mapOf("name" to "feature-a"),
                expected = "delete 1 feature-a false",
            ) { tag(TAG_DELETE_CONFIRM).performClick() },
            Case("RenameBranch", mapOf("name" to "feature-a"), expected = "rename 1 feature-a feature-c") {
                typeInto(TAG_NAME, "feature-c")
                tag(TAG_RENAME_CONFIRM).performClick()
            },
            Case("CreateBranch", mapOf("initial_name" to "topic", "target_sha" to "abc1234"), expected = "create 1 topic abc1234") {
                tag(TAG_CREATE_CONFIRM).performClick()
            },
            Case("CreateTag", mapOf("sha" to "abc1234"), expected = "tag 1 v1.0 abc1234") {
                onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag(TAG_TAG_NAME))).performTextInput("v1.0")
                tag(TAG_TAG_CONFIRM).performClick()
            },
            Case(
                "ConfirmForcePush",
                mapOf("upstream_branch" to "origin/main"),
                expected = "push 1 true",
            ) { tag(TAG_FORCE_CONFIRM).performClick() },
            Case("PushNeedsPull", expected = "pull 1"),
            Case("UpstreamAlreadyExists", mapOf("existing_url" to "https://example.com/x.git"), expected = "close"),
            Case("StashAndSwitchBranch", mapOf("branch" to "feature-a"), expected = "checkout 1 feature-a stash") {
                tag(TAG_SWITCH_CONFIRM).performClick()
            },
            Case("ConfirmOverwriteStash", mapOf("branch" to "feature-a"), expected = "checkout 1 feature-a stash") {
                onNodeWithText("Overwrite").performClick()
            },
            Case("ConfirmSwitchBranch", mapOf("branch" to "feature-a"), expected = "checkout 1 feature-a null") {
                onAllNodesWithText("Switch branch").onFirst().performClick()
            },
            Case("ConfirmDiscardStash", expected = "dropStash 1"),
            Case(
                "CheckoutCommit",
                mapOf("sha" to "abc1234"),
                expected = "checkoutCommit 1 abc1234",
            ) { tag(TAG_CHECKOUT_CONFIRM).performClick() },
            Case("WarnLocalChangesBeforeUndo", expected = "undo 1"),
            Case("ResetToCommit", mapOf("sha" to "abc1234"), expected = "reset 1 abc1234") { tag(TAG_RESET_CONFIRM).performClick() },
            Case(
                "UnknownAuthors",
                mapOf("summary" to "Fix", "description" to "body"),
                mapOf("usernames" to listOf("ghost")),
                expected = "commit 1 Fix|body",
            ),
            Case("LocalChangesOverwritten", mapOf("retry" to "Pull"), mapOf("files" to listOf("a.txt")), expected = "retry"),
            Case("ConfirmRemoveRepository", expected = "remove 1"),
            Case("ExternalEditorError", mapOf("message" to "No editor"), expected = "close"),
            Case("ShellError", mapOf("message" to "No shell"), expected = "close"),
            Case("MergeBranch", mapOf("squash" to "false"), expected = "merge 1 feature-a false") {
                tag("${TAG_CHOOSE_BRANCH}feature-a").performClick()
            },
            Case("MultiCommitOperation", mapOf("flow" to "1"), expected = "mco 1") { waitForIdle() },
            Case("CICheckRunRerun", mapOf("git_ref" to "main"), mapOf("checks" to listOf("build")), expected = "close"),
            Case(
                "PullRequestChecksFailed",
                mapOf("number" to "12", "title" to "Fix"),
                mapOf("checks" to listOf("lint")),
                expected = "close",
            ),
            Case("GenericGitAuthentication", mapOf("host" to "git.example.com", "username" to "wasi"), expected = "auth wasi s3cret") {
                onAllNodes(hasSetTextAction())[1].performTextInput("s3cret")
                tag(TAG_AUTH_CONFIRM).performClick()
            },
            Case(
                "PublishRepository",
                expected = "publish 1 demo private=true https://api.github.com null",
            ) { tag(TAG_PUBLISH_CONFIRM).performClick() },
            Case("Foo", mapOf("x" to "y"), expected = "close"),
        ).map { arrayOf(it) }

        private val Context = PopupContext(
            branches = BranchesVm(
                repo = 1u,
                current = "main",
                detachedSha = null,
                defaultBranch = "main",
                recent = emptyList(),
                branches = listOf(
                    BranchVm("main", false, true, null, null),
                    BranchVm("feature-a", false, false, null, null),
                ),
                ahead = null,
                behind = null,
                sync = SyncActionVm.FETCH,
                syncProgressTitle = null,
                syncProgress = null,
                lastFetchedAt = null,
                stashCount = 1u,
                stashOnCurrentBranch = true,
            ),
            accounts = listOf(AccountVm(endpoint = "https://api.github.com", login = "wasi-master", name = null, avatarUrl = null, avatarPath = null, host = "github.com", emails = emptyList())),
            repositoryName = "demo",
        )

        private fun recorder(sent: MutableList<String>) = object : PopupActions {
            override fun close() {
                sent += "close"
            }

            override fun discardChanges(repo: ULong, paths: List<String>) {
                sent += "discard $repo $paths"
            }

            override fun deleteBranch(repo: ULong, name: String, includeRemote: Boolean) {
                sent += "delete $repo $name $includeRemote"
            }

            override fun renameBranch(repo: ULong, old: String, new: String) {
                sent += "rename $repo $old $new"
            }

            override fun createBranch(repo: ULong, name: String, startPoint: String?) {
                sent += "create $repo $name $startPoint"
            }

            override fun createTag(repo: ULong, name: String, sha: String, message: String) {
                sent += "tag $repo $name $sha$message"
            }

            override fun push(repo: ULong, force: Boolean) {
                sent += "push $repo $force"
            }

            override fun pull(repo: ULong) {
                sent += "pull $repo"
            }

            override fun fetch(repo: ULong) {
                sent += "fetch $repo"
            }

            override fun checkoutBranch(repo: ULong, name: String, strategy: String?) {
                sent += "checkout $repo $name $strategy"
            }

            override fun dropStash(repo: ULong) {
                sent += "dropStash $repo"
            }

            override fun stashAllChanges(repo: ULong) {
                sent += "stash $repo"
            }

            override fun checkoutCommit(repo: ULong, sha: String) {
                sent += "checkoutCommit $repo $sha"
            }

            override fun resetToCommit(repo: ULong, sha: String) {
                sent += "reset $repo $sha"
            }

            override fun undoCommit(repo: ULong) {
                sent += "undo $repo"
            }

            override fun commit(repo: ULong, summary: String, description: String) {
                sent += "commit $repo $summary|$description"
            }

            override fun removeRepository(repo: ULong) {
                sent += "remove $repo"
            }

            override fun mergeBranch(repo: ULong, branch: String, squash: Boolean) {
                sent += "merge $repo $branch $squash"
            }

            override fun submitGenericAuth(username: String, password: String) {
                sent += "auth $username $password"
            }

            override fun retry() {
                sent += "retry"
            }

            override fun publishRepository(
                repo: ULong,
                name: String,
                description: String,
                private: Boolean,
                endpoint: String,
                org: String?,
            ) {
                sent += "publish $repo $name private=$private $endpoint $org"
            }
        }
    }
}
