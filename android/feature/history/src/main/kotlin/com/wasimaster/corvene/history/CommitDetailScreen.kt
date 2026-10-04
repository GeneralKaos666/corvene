package com.wasimaster.corvene.history

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.common.relativeTime
import com.wasimaster.corvene.design.Avatar
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.FilePath
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonColored
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.ffi.gen.CommitDetailVm
import com.wasimaster.corvene.ffi.gen.CommitFileVm
import com.wasimaster.corvene.ffi.gen.FileStatusVm

/** What the commit detail sends back. */
interface CommitDetailActions {
    /** A file tapped: the engine loads its diff (`selectCommitFile`). */
    fun selectFile(path: String)

    fun copySha(sha: String)
}

/**
 * GHD's SelectedCommit (GitHub Mobile's commit view): the summary with the
 * body behind "Show more", the author and relative date, the SHA with a copy
 * button, "+adds −dels" over the changed files, then the files with their
 * status icons; the selected one highlighted. A range selection names how
 * many commits it covers.
 */
@Composable
fun CommitDetailScreen(
    detail: CommitDetailVm,
    actions: CommitDetailActions,
    modifier: Modifier = Modifier,
    now: Long = System.currentTimeMillis(),
    contentPadding: PaddingValues = PaddingValues(),
) {
    val colors = CorveneTheme.colors
    LazyColumn(modifier.fillMaxSize().background(colors.bgCanvas).testTag(TAG_DETAIL), contentPadding = contentPadding) {
        item(key = "header", contentType = "header") { Header(detail, actions, now) }
        item(key = "stats", contentType = "stats") {
            Row(
                Modifier.fillMaxWidth().background(colors.bgSubtle).padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    pluralStringResource(R.plurals.hist_files_changed, detail.files.size, detail.files.size),
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.textSecondary,
                )
                Text("+${detail.linesAdded}", style = CorveneTheme.textStyles.codeSmall, color = colors.success.fg)
                Text("−${detail.linesDeleted}", style = CorveneTheme.textStyles.codeSmall, color = colors.danger.fg)
            }
            HorizontalDivider(color = colors.borderMuted)
        }
        items(detail.files, key = { it.path }, contentType = { "file" }) { file -> FileRow(file, actions) }
    }
}

@Composable
private fun Header(detail: CommitDetailVm, actions: CommitDetailActions, now: Long) {
    val colors = CorveneTheme.colors
    var expanded by rememberSaveable(detail.shas) { mutableStateOf(false) }
    Column(
        Modifier.fillMaxWidth().padding(horizontal = CorveneTheme.metrics.gutter, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (detail.shas.size > 1) {
            Text(
                pluralStringResource(R.plurals.hist_range, detail.shas.size, detail.shas.size),
                style = MaterialTheme.typography.labelLarge,
                color = colors.accent.fg,
            )
        }
        Text(
            detail.summary,
            style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.SemiBold),
            color = colors.textPrimary,
            modifier = Modifier.testTag(TAG_SUMMARY),
        )
        val body = detail.body.trim()
        if (body.isNotEmpty()) {
            Text(
                body,
                style = CorveneTheme.textStyles.codeSmall,
                color = colors.textSecondary,
                maxLines = if (expanded) Int.MAX_VALUE else BODY_LINES,
                overflow = TextOverflow.Ellipsis,
            )
            if (body.lines().size > BODY_LINES || body.length > BODY_CHARS) {
                PrimerButton(
                    stringResource(if (expanded) R.string.hist_show_less else R.string.hist_show_more),
                    { expanded = !expanded },
                    variant = PrimerButtonVariant.Link,
                    modifier = Modifier.testTag(TAG_EXPAND),
                )
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Avatar(detail.authorName, size = 20.dp)
            Text(
                stringResource(R.string.hist_authored, detail.authorName, relativeTime(detail.authoredAt, now)),
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodySmall,
                color = colors.textSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        val sha = detail.shas.firstOrNull().orEmpty()
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Octicon(Octicons.GitCommit, null, tint = OcticonTint.Secondary)
            Text(sha.take(SHORT_SHA), style = CorveneTheme.textStyles.codeSmall, color = colors.textPrimary)
            PrimerIconButton(
                Octicons.Copy,
                stringResource(R.string.hist_copy_sha),
                { actions.copySha(sha) },
                tint = OcticonTint.Secondary,
                modifier = Modifier.testTag(TAG_COPY),
            )
        }
    }
}

@Composable
private fun FileRow(file: CommitFileVm, actions: CommitDetailActions) {
    val colors = CorveneTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .semantics { selected = file.selected }
            .background(if (file.selected) colors.bgSelected else colors.bgCanvas)
            .clickable(role = Role.Button) { actions.selectFile(file.path) }
            .heightIn(min = CorveneTheme.metrics.rowHeight.coerceAtMost(48.dp))
            .padding(horizontal = CorveneTheme.metrics.gutter)
            .testTag("$TAG_FILE${file.path}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        StatusIcon(file.status)
        FilePath(file.path, Modifier.weight(1f), oldPath = file.oldPath, style = MaterialTheme.typography.bodyMedium)
    }
}

/** GHD's file status glyphs in the semantic colours. */
@Composable
private fun StatusIcon(status: FileStatusVm) {
    val colors = CorveneTheme.colors
    val (icon, color, label) = when (status) {
        FileStatusVm.NEW, FileStatusVm.UNTRACKED, FileStatusVm.COPIED -> Triple(
            Octicons.DiffAdded,
            colors.fileNew,
            R.string.hist_status_new,
        )
        FileStatusVm.MODIFIED -> Triple(Octicons.DiffModified, colors.fileModified, R.string.hist_status_modified)
        FileStatusVm.DELETED -> Triple(Octicons.DiffRemoved, colors.fileDeleted, R.string.hist_status_deleted)
        FileStatusVm.RENAMED -> Triple(Octicons.DiffRenamed, colors.fileRenamed, R.string.hist_status_renamed)
        FileStatusVm.CONFLICTED -> Triple(Octicons.Alert, colors.fileConflicted, R.string.hist_status_conflicted)
    }
    OcticonColored(icon, stringResource(label), color)
}

private const val SHORT_SHA = 7
private const val BODY_LINES = 3
private const val BODY_CHARS = 240

const val TAG_DETAIL = "hist_detail"
const val TAG_SUMMARY = "hist_summary"
const val TAG_EXPAND = "hist_expand"
const val TAG_COPY = "hist_copy"
const val TAG_FILE = "hist_file_"
