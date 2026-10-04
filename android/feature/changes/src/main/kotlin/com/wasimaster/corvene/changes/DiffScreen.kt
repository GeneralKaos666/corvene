package com.wasimaster.corvene.changes

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.rememberScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableFloatState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuDivider
import com.wasimaster.corvene.design.ActionMenuGroupHeader
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.Blankslate
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DiffPalette
import com.wasimaster.corvene.design.FilePath
import com.wasimaster.corvene.design.OcticonIcon
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerCheckbox
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.design.SegmentedControl
import com.wasimaster.corvene.design.Spinner
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.design.isCompactWidth
import com.wasimaster.corvene.ffi.gen.DiffHeaderVm
import com.wasimaster.corvene.ffi.gen.DiffKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowKindVm
import com.wasimaster.corvene.ffi.gen.DiffRowVm
import com.wasimaster.corvene.ffi.gen.IncludeVm
import kotlinx.coroutines.flow.collectLatest
import kotlin.math.roundToInt

/** What the diff sends back. */
interface DiffActions {
    fun toggleFileIncluded()

    /** Tap on an added or deleted line: in or out of the next commit. */
    fun toggleLine(index: Int)

    fun setHideWhitespace(hide: Boolean)
}

/**
 * The selected file's diff: a header (path, +/− counts, the include box, the
 * gear menu: wrap lines, hide whitespace, unified/split on wide screens) and
 * the unified diff, its rows paged from the engine by [pager] and highlighted
 * through [cache]. Lines wrap by default on compact widths (GitHub Mobile)
 * and scroll sideways together otherwise. Non-text diffs show a notice.
 */
@Composable
fun DiffScreen(
    header: DiffHeaderVm,
    pager: DiffPager?,
    cache: DiffLineCache,
    hideWhitespace: Boolean,
    actions: DiffActions,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    val compact = isCompactWidth()
    var wrap by rememberSaveable { mutableStateOf(compact) }
    var showLarge by rememberSaveable(header.generation) { mutableStateOf(false) }
    Column(modifier.fillMaxSize().background(CorveneTheme.diff.canvas)) {
        DiffHeaderRow(header, wrap, { wrap = it }, hideWhitespace, actions, showSplit = !compact)
        val path = header.path
        when {
            path == null -> Notice(Octicons.FileDiff, stringResource(R.string.chg_diff_none), null, contentPadding)
            header.kind == DiffKindVm.LOADING -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Spinner(label = stringResource(R.string.chg_diff_loading))
            }
            header.kind == DiffKindVm.BINARY -> Notice(Octicons.FileBinary, stringResource(R.string.chg_diff_binary), null, contentPadding)
            header.kind == DiffKindVm.IMAGE -> Notice(
                Octicons.FileMedia,
                stringResource(R.string.chg_diff_image),
                stringResource(R.string.chg_diff_image_later),
                contentPadding,
            )
            header.kind == DiffKindVm.TOO_LARGE -> Notice(
                Octicons.Alert,
                stringResource(R.string.chg_diff_too_large),
                stringResource(R.string.chg_diff_too_large_body),
                contentPadding,
            )
            header.kind == DiffKindVm.SUBMODULE ->
                Notice(Octicons.FileSubmodule, stringResource(R.string.chg_diff_submodule), null, contentPadding)
            header.kind == DiffKindVm.EMPTY || header.rowCount == 0u ->
                Notice(Octicons.File, stringResource(R.string.chg_diff_empty), null, contentPadding)
            header.kind == DiffKindVm.LARGE_TEXT && !showLarge -> Notice(
                Octicons.Alert,
                stringResource(R.string.chg_diff_large),
                stringResource(R.string.chg_diff_large_body),
                contentPadding,
            ) { PrimerButton(stringResource(R.string.chg_diff_show), { showLarge = true }) }
            pager != null -> DiffRows(pager, cache, wrap, actions, contentPadding)
        }
    }
}

@Composable
private fun DiffHeaderRow(
    header: DiffHeaderVm,
    wrap: Boolean,
    onWrap: (Boolean) -> Unit,
    hideWhitespace: Boolean,
    actions: DiffActions,
    showSplit: Boolean,
) {
    val colors = CorveneTheme.colors
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().background(colors.bgSubtle)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 4.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            val path = header.path
            if (path != null) {
                PrimerCheckbox(
                    header.include.toToggleable(),
                    onClick = actions::toggleFileIncluded,
                    contentDescription = stringResource(R.string.chg_include_file, path),
                    modifier = Modifier.testTag(TAG_DIFF_INCLUDE),
                )
                FilePath(path, Modifier.weight(1f), style = CorveneTheme.textStyles.codeSmall)
            } else {
                Box(Modifier.weight(1f))
            }
            if (header.linesAdded > 0u || header.linesDeleted > 0u) LineStats(header.linesAdded, header.linesDeleted)
            Box {
                PrimerIconButton(Octicons.Gear, stringResource(R.string.chg_diff_options), { menu = true })
                ActionMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    if (showSplit) {
                        ActionMenuGroupHeader(stringResource(R.string.chg_diff_view))
                        var split by remember { mutableIntStateOf(0) }
                        SegmentedControl(
                            listOf(stringResource(R.string.chg_diff_unified), stringResource(R.string.chg_diff_split)),
                            split,
                            { split = it },
                            Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                        )
                        if (split == 1) {
                            Text(
                                stringResource(R.string.chg_diff_split_later),
                                Modifier.padding(horizontal = 16.dp).width(220.dp),
                                style = MaterialTheme.typography.bodySmall,
                                color = colors.textSecondary,
                            )
                        }
                        ActionMenuDivider()
                    }
                    ActionMenuItem(stringResource(R.string.chg_diff_wrap), { onWrap(!wrap) }, checked = wrap)
                    SwitchRow(
                        stringResource(R.string.chg_diff_hide_whitespace),
                        checked = hideWhitespace,
                        onCheckedChange = actions::setHideWhitespace,
                        modifier = Modifier.width(260.dp),
                    )
                }
            }
        }
        HorizontalDivider(color = colors.borderMuted)
    }
}

@Composable
internal fun LineStats(added: UInt, deleted: UInt, modifier: Modifier = Modifier) {
    val colors = CorveneTheme.colors
    val description = stringResource(R.string.chg_line_stats, added.toInt(), deleted.toInt())
    Row(
        modifier.semantics(mergeDescendants = true) { contentDescription = description },
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text("+$added", style = CorveneTheme.textStyles.codeSmall, color = colors.success.fg)
        Text("−$deleted", style = CorveneTheme.textStyles.codeSmall, color = colors.danger.fg)
    }
}

@Composable
private fun Notice(
    icon: OcticonIcon,
    title: String,
    description: String?,
    contentPadding: PaddingValues,
    action: (@Composable () -> Unit)? = null,
) {
    Box(Modifier.fillMaxSize().padding(contentPadding), contentAlignment = Alignment.Center) {
        Blankslate(icon = icon, title = title, description = description, primaryAction = action)
    }
}

@Composable
private fun DiffRows(pager: DiffPager, cache: DiffLineCache, wrap: Boolean, actions: DiffActions, contentPadding: PaddingValues) {
    val listState = rememberLazyListState()
    LaunchedEffect(pager, listState) {
        snapshotFlow { listState.firstVisibleItemIndex to (listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) }
            .collectLatest { (first, last) -> pager.ensure(first, maxOf(first, last)) }
    }
    val palette = CorveneTheme.diff
    val code = CorveneTheme.textStyles.code
    val density = LocalDensity.current
    val measurer = rememberTextMeasurer()
    val charWidth = remember(code, density) { measurer.measure("0", code).size.width.toFloat() }
    val rowHeight = with(density) { code.lineHeight.toDp() }
    val gutter = if (pager.rowCount >= GUTTER_WIDE_ROWS) 44.dp else 36.dp
    val offset = remember(pager) { mutableFloatStateOf(0f) }
    // resolved once, not per row: the list composes dozens of rows per fling frame
    val labels = RowLabels(stringResource(R.string.chg_line_include), stringResource(R.string.chg_line_exclude))
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val viewport = with(density) { (maxWidth - gutter * 2 - SIGN_WIDTH).toPx() }
        val maxOffset = (pager.maxColumns * charWidth - viewport + with(density) { 16.dp.toPx() }).coerceAtLeast(0f)
        val scroll = rememberScrollableState { delta ->
            val before = offset.floatValue
            offset.floatValue = (before - delta).coerceIn(0f, maxOffset)
            before - offset.floatValue
        }
        LazyColumn(
            state = listState,
            contentPadding = contentPadding,
            modifier = Modifier
                .fillMaxSize()
                .testTag(TAG_DIFF_LIST)
                .then(if (wrap) Modifier else Modifier.scrollable(scroll, Orientation.Horizontal, reverseDirection = false)),
        ) {
            items(pager.rowCount, key = { it }, contentType = { pager.row(it)?.kind ?: DiffRowKindVm.CONTEXT }) { index ->
                val row = pager.row(index)
                if (row == null) {
                    Box(Modifier.fillMaxWidth().height(rowHeight))
                } else {
                    DiffRow(
                        row,
                        cache.line(pager.generation, row, palette),
                        palette,
                        code,
                        gutter,
                        if (wrap) null else rowHeight,
                        if (wrap) null else offset,
                        labels,
                        onToggle = { actions.toggleLine(row.index.toInt()) },
                    )
                }
            }
        }
    }
}

/** One fixed-height row (or a wrapped one): two line-number gutters, the sign, the text. */
@Composable
private fun DiffRow(
    row: DiffRowVm,
    text: AnnotatedString,
    palette: DiffPalette,
    style: TextStyle,
    gutter: Dp,
    height: Dp?,
    offset: MutableFloatState?,
    labels: RowLabels,
    onToggle: () -> Unit,
) {
    val changeable = row.kind == DiffRowKindVm.ADD || row.kind == DiffRowKindVm.DELETE
    val (bg, gutterBg, sign, signColor) = when (row.kind) {
        DiffRowKindVm.ADD -> RowLook(palette.addBg, palette.addGutterBg, "+", palette.addSign)
        DiffRowKindVm.DELETE -> RowLook(palette.delBg, palette.delGutterBg, "−", palette.delSign)
        DiffRowKindVm.HUNK -> RowLook(palette.hunkBg, palette.hunkBg, "", palette.hunkText)
        DiffRowKindVm.CONTEXT -> RowLook(palette.canvas, palette.gutterBg, "", palette.lineNumber)
    }
    val selected = changeable && row.selected
    val numberColor = if (selected) palette.selectedText else palette.lineNumber
    val numberBg = if (selected) palette.selectedBg else gutterBg
    val toggleLabel = if (row.selected) labels.exclude else labels.include
    val sized = if (height != null) Modifier.height(height) else Modifier.height(IntrinsicSize.Min)
    Row(
        Modifier
            .fillMaxWidth()
            .then(sized)
            .background(bg)
            .then(
                if (changeable) {
                    Modifier
                        .semantics { this.selected = row.selected }
                        .clickable(role = Role.Checkbox, onClickLabel = toggleLabel, onClick = onToggle)
                } else {
                    Modifier
                },
            )
            .testTag("$TAG_DIFF_ROW${row.index}"),
    ) {
        val numbers = style.copy(color = numberColor, textAlign = TextAlign.End)
        Text(
            row.oldLine?.toString().orEmpty(),
            Modifier.width(gutter).fillMaxHeight().background(numberBg).padding(end = 4.dp),
            style = numbers,
            maxLines = 1,
        )
        Text(
            row.newLine?.toString().orEmpty(),
            Modifier.width(gutter).fillMaxHeight().background(numberBg).padding(end = 4.dp),
            style = numbers,
            maxLines = 1,
        )
        Text(sign, Modifier.width(SIGN_WIDTH), style = style.copy(color = signColor, textAlign = TextAlign.Center), maxLines = 1)
        val textColor = if (row.kind == DiffRowKindVm.HUNK) palette.hunkText else palette.text
        if (offset == null) {
            Text(text, Modifier.weight(1f).padding(end = 8.dp), style = style.copy(color = textColor))
        } else {
            Box(Modifier.weight(1f).fillMaxHeight().clipToBounds()) {
                Text(
                    text,
                    Modifier
                        .wrapContentWidth(Alignment.Start, unbounded = true)
                        .offset { IntOffset(-offset.floatValue.roundToInt(), 0) },
                    style = style.copy(color = textColor),
                    maxLines = 1,
                    softWrap = false,
                )
            }
        }
    }
}

private data class RowLook(
    val bg: Color,
    val gutterBg: Color,
    val sign: String,
    val signColor: Color,
)

internal fun IncludeVm.toToggleable(): ToggleableState = when (this) {
    IncludeVm.ALL -> ToggleableState.On
    IncludeVm.PARTIAL -> ToggleableState.Indeterminate
    IncludeVm.NONE -> ToggleableState.Off
}

private val SIGN_WIDTH = 16.dp
private const val GUTTER_WIDE_ROWS = 1000

const val TAG_DIFF_LIST = "chg_diff_list"
const val TAG_DIFF_ROW = "chg_diff_row_"
const val TAG_DIFF_INCLUDE = "chg_diff_include"

/** The accessibility labels of a changeable row, resolved once per list. */
@Immutable
private data class RowLabels(val include: String, val exclude: String)
