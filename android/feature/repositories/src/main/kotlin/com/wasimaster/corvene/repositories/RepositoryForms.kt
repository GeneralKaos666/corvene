package com.wasimaster.corvene.repositories

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTopAppBar
import com.wasimaster.corvene.ffi.gen.CloneableRepositoryVm

/**
 * The frame of the full-screen forms (Clone, Add, Create; GHD's dialogs,
 * full screen on a phone): a top bar with close and the primary action, then
 * the form, kept above the keyboard.
 */
@Composable
internal fun RepositoryForm(
    title: String,
    confirm: String,
    confirmEnabled: Boolean,
    onConfirm: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas).imePadding()) {
        PrimerTopAppBar(
            title = title,
            onBack = onClose,
            backDescription = stringResource(R.string.repo_close),
            actions = {
                PrimerButton(
                    confirm,
                    onConfirm,
                    Modifier.padding(end = 8.dp).testTag(TAG_CONFIRM),
                    variant = PrimerButtonVariant.Primary,
                    enabled = confirmEnabled,
                )
            },
        )
        Column(
            Modifier
                .weight(1f)
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom)),
            content = content,
        )
    }
}

/** The local path row: where a clone or a new repository goes; tap = choose another folder. */
@Composable
internal fun LocalPathRow(
    path: String,
    onChoose: () -> Unit,
    modifier: Modifier = Modifier,
    label: String = stringResource(R.string.repo_local_path),
) {
    ActionListItem(
        title = label,
        description = path,
        onClick = onChoose,
        modifier = modifier.testTag(TAG_LOCAL_PATH),
        leading = { IconTile(Octicons.FileDirectory, CorveneTheme.colors.accent) },
        trailing = {
            Text(stringResource(R.string.repo_choose), color = CorveneTheme.colors.textLink, style = MaterialTheme.typography.labelLarge)
        },
    )
}

@Composable
internal fun FormCaption(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
        style = MaterialTheme.typography.bodySmall,
        color = CorveneTheme.colors.textSecondary,
    )
}

/** Fields stacked with the form's gutter. */
@Composable
internal fun FormFields(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        content = content,
    )
}

/**
 * What the Clone dialog's URL field accepts, as git needs it: a URL or
 * `user@host:path` as typed, `owner/name` as a GitHub.com URL (GHD resolves
 * that through the API; a GitHub.com address is what it finds).
 */
fun cloneUrlFor(input: String): String {
    val text = input.trim()
    return if (SHORTHAND.matches(text)) "https://github.com/$text" else text
}

/** The folder name a clone of [url] gets: the last path segment without `.git`. */
fun repositoryNameOf(url: String): String =
    url.trim().trimEnd('/').substringAfterLast('/').substringAfterLast(':').removeSuffix(".git")
        .replace(Regex("[^A-Za-z0-9._-]"), "-")

private val SHORTHAND = Regex("[A-Za-z0-9-]+/[A-Za-z0-9._-]+")

const val TAG_CONFIRM = "repo_confirm"
const val TAG_LOCAL_PATH = "repo_local_path"

private fun sampleRepository(name: String, private: Boolean, fork: Boolean): CloneableRepositoryVm {
    val url = "https://github.com/octocat/$name"
    return CloneableRepositoryVm("octocat", name, "$url.git", url, private, fork, "main")
}

internal val SampleCloneable = listOf(
    sampleRepository("Hello-World", private = false, fork = false),
    sampleRepository("Spoon-Knife", private = false, fork = true),
    sampleRepository("secret-plans", private = true, fork = false),
)

/** Does nothing: previews and screenshots. */
internal object NoCloneActions : CloneActions {
    override fun tab(tab: CloneTab) = Unit

    override fun url(url: String) = Unit

    override fun pick(repository: CloneableRepositoryVm) = Unit

    override fun filter(text: String) = Unit

    override fun choosePath() = Unit

    override fun shallow(on: Boolean) = Unit

    override fun signIn() = Unit

    override fun clone() = Unit

    override fun close() = Unit
}
