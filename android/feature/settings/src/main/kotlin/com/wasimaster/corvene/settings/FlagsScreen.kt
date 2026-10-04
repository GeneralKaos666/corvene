package com.wasimaster.corvene.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.FilterField
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Label
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerChip
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.design.PrimerIconButton
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.PrimerSwitch
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.SegmentedControl
import com.wasimaster.corvene.design.SelectPanel
import com.wasimaster.corvene.design.SwitchRow
import com.wasimaster.corvene.ffi.gen.FlagVm
import com.wasimaster.corvene.ffi.gen.FlagsVm
import com.wasimaster.corvene.ffi.gen.PresetVm

/** What the Flags screen sends: `setFlagBySlug`, `resetFlag`, `applyPreset`, `resetAllFlags`, `relaunch`. */
interface FlagActions {
    fun set(slug: String, value: String)

    fun reset(slug: String)

    fun applyPreset(slug: String)

    fun resetAll()

    fun relaunch()
}

/** The list's display state (desktop dialog: the search, All / On / Off, Show bug fixes), kept by the route. */
@Immutable
data class FlagsView(val query: String = "", val filter: FlagFilter = FlagFilter.All, val showBugFixes: Boolean = false)

/**
 * Settings › Flags, the desktop Flags dialog on a phone: every switchable
 * deviation from GitHub Desktop grouped by category, searchable, narrowed
 * by All / On / Off and "Show bug fixes"; each flag with its control (switch,
 * menu, stepper and slider, text with the engine's validation [errors]),
 * an "overridden" dot and Reset; the preset (a sheet of the four with their
 * descriptions, confirmed), Reset all (confirmed), and the Relaunch bar
 * once a flag that needs a restart changed.
 */
@Composable
fun FlagsScreen(
    flags: FlagsVm,
    view: FlagsView,
    onView: (FlagsView) -> Unit,
    errors: Map<String, String>,
    actions: FlagActions,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    var presetsOpen by rememberSaveable { mutableStateOf(false) }
    var confirmPreset by rememberSaveable { mutableStateOf<String?>(null) }
    var confirmResetAll by rememberSaveable { mutableStateOf(false) }
    val groups = remember(flags, view) { flags.visibleGroups(view.query, view.filter, view.showBugFixes) }
    val colors = CorveneTheme.colors
    val gutter = CorveneTheme.metrics.gutter
    LazyColumn(modifier.fillMaxSize().background(colors.bgCanvas).testTag(TAG_FLAGS), contentPadding = contentPadding) {
        if (flags.restartPending) {
            item(key = "restart") {
                Flash(
                    stringResource(R.string.set_flags_restart),
                    variant = FlashVariant.Warning,
                    flush = true,
                    modifier = Modifier.testTag(TAG_FLAGS_RESTART),
                    action = {
                        PrimerButton(
                            stringResource(R.string.set_flags_relaunch),
                            actions::relaunch,
                            Modifier.testTag(TAG_FLAGS_RELAUNCH),
                            variant = PrimerButtonVariant.Primary,
                        )
                    },
                )
            }
        }
        item(key = "preset") {
            ActionListItem(
                stringResource(R.string.set_flags_preset),
                description = if (flags.anyOverridden) {
                    stringResource(R.string.set_flags_custom, flags.presetTitle)
                } else {
                    flags.presetTitle
                },
                onClick = { presetsOpen = true },
                chevron = true,
                modifier = Modifier.testTag(TAG_FLAGS_PRESET),
                trailing = {
                    PrimerButton(
                        stringResource(R.string.set_flags_reset_all),
                        { confirmResetAll = true },
                        Modifier.testTag(TAG_FLAGS_RESET_ALL),
                        variant = PrimerButtonVariant.Danger,
                        enabled = flags.anyOverridden,
                    )
                },
            )
        }
        item(key = "toolbar") {
            Column(Modifier.padding(horizontal = gutter, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterField(
                    view.query,
                    { onView(view.copy(query = it)) },
                    Modifier.fillMaxWidth().testTag(TAG_FLAGS_QUERY),
                    placeholder = stringResource(R.string.set_flags_filter),
                    clearDescription = stringResource(R.string.set_flags_clear),
                )
                SegmentedControl(
                    listOf(
                        stringResource(R.string.set_flags_all),
                        stringResource(R.string.set_flags_on),
                        stringResource(R.string.set_flags_off),
                    ),
                    selectedIndex = view.filter.ordinal,
                    onSelect = { onView(view.copy(filter = FlagFilter.entries[it])) },
                    modifier = Modifier.testTag(TAG_FLAGS_STATE),
                    fill = true,
                )
            }
            SwitchRow(
                stringResource(R.string.set_flags_bug_fixes),
                view.showBugFixes,
                { onView(view.copy(showBugFixes = it)) },
                Modifier.testTag(TAG_FLAGS_BUG_FIXES),
            )
        }
        if (groups.isEmpty()) {
            item(key = "none") {
                Text(
                    stringResource(R.string.set_flags_none),
                    Modifier.fillMaxWidth().padding(24.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.textSecondary,
                )
            }
        }
        groups.forEach { group ->
            item(key = "category:${group.category}", contentType = "header") { ActionListGroupHeader(group.category) }
            items(group.flags, key = { it.slug }, contentType = { it.kind }) { flag ->
                FlagRow(flag, errors[flag.slug], actions)
                ActionListDivider(leadingInset = false)
            }
        }
    }
    if (presetsOpen) {
        PresetPanel(
            flags.presets,
            current = flags.preset,
            onPick = { slug ->
                presetsOpen = false
                if (slug != flags.preset || flags.anyOverridden) confirmPreset = slug
            },
            onDismissRequest = { presetsOpen = false },
        )
    }
    confirmPreset?.let { slug ->
        val title = flags.presets.firstOrNull { it.slug == slug }?.title ?: slug
        ConfirmDialog(
            title = stringResource(R.string.set_flags_apply_preset_title, title),
            body = stringResource(R.string.set_flags_apply_preset_body),
            confirm = stringResource(R.string.set_flags_apply),
            danger = false,
            onConfirm = {
                confirmPreset = null
                actions.applyPreset(slug)
            },
            onDismiss = { confirmPreset = null },
        )
    }
    if (confirmResetAll) {
        ConfirmDialog(
            title = stringResource(R.string.set_flags_reset_all_title),
            body = stringResource(R.string.set_flags_reset_all_body, flags.presetTitle),
            confirm = stringResource(R.string.set_flags_reset_all),
            danger = true,
            onConfirm = {
                confirmResetAll = false
                actions.resetAll()
            },
            onDismiss = { confirmResetAll = false },
        )
    }
}

@Composable
private fun ConfirmDialog(title: String, body: String, confirm: String, danger: Boolean, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    PrimerDialog(
        title = title,
        onDismissRequest = onDismiss,
        closeDescription = stringResource(R.string.set_close),
        confirmButton = {
            PrimerButton(
                confirm,
                onConfirm,
                Modifier.testTag(TAG_FLAGS_CONFIRM),
                variant = if (danger) PrimerButtonVariant.Danger else PrimerButtonVariant.Primary,
            )
        },
        dismissButton = { PrimerButton(stringResource(R.string.set_cancel), onDismiss, variant = PrimerButtonVariant.Invisible) },
    ) {
        Text(body, color = CorveneTheme.colors.textPrimary)
    }
}

/** The four presets with their descriptions, the current one selected. */
@Composable
private fun PresetPanel(presets: List<PresetVm>, current: String, onPick: (String) -> Unit, onDismissRequest: () -> Unit) {
    SelectPanel(
        title = stringResource(R.string.set_flags_presets),
        onDismissRequest = onDismissRequest,
        closeDescription = stringResource(R.string.set_close),
    ) {
        item {
            Column(Modifier.selectableGroup()) {
                presets.forEach { preset ->
                    RadioRow(
                        preset.title,
                        selected = preset.slug == current,
                        onClick = { onPick(preset.slug) },
                        caption = preset.description,
                        modifier = Modifier.testTag("$TAG_FLAGS_PRESET_ITEM${preset.slug}"),
                    )
                }
            }
        }
    }
}

/**
 * One flag: its title (a dot when it differs from the preset), the control
 * of its kind, Reset when overridden; the summary under it, and on a tap
 * GitHub Desktop's behaviour and value. An unavailable flag is greyed and
 * takes no input.
 */
@Composable
private fun FlagRow(flag: FlagVm, error: String?, actions: FlagActions) {
    val colors = CorveneTheme.colors
    var expanded by rememberSaveable(flag.slug) { mutableStateOf(false) }
    val enabled = flag.available
    Column(
        Modifier
            .fillMaxWidth()
            .testTag("$TAG_FLAG${flag.slug}")
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // the title shows GitHub Desktop's side on a tap (the controls keep their own semantics)
            Row(
                Modifier.weight(1f).clickable { expanded = !expanded },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (flag.overridden) {
                    val overridden = stringResource(R.string.set_flags_overridden)
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(colors.accent.emphasis)
                            .semantics { contentDescription = overridden }
                            .testTag("$TAG_FLAG_DOT${flag.slug}"),
                    )
                }
                Text(
                    flag.title,
                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                    color = if (enabled) colors.textPrimary else colors.textDisabled,
                )
            }
            if (flag.overridden && enabled) {
                PrimerIconButton(
                    Octicons.Undo,
                    stringResource(R.string.set_flags_reset_flag, flag.title),
                    { actions.reset(flag.slug) },
                    Modifier.size(36.dp).testTag("$TAG_FLAG_RESET${flag.slug}"),
                    tint = OcticonTint.Secondary,
                )
            }
            when (flag.kind) {
                KIND_TOGGLE -> PrimerSwitch(
                    checked = flag.value == "true",
                    onCheckedChange = { actions.set(flag.slug, it.toString()) },
                    modifier = Modifier.testTag("$TAG_FLAG_CONTROL${flag.slug}"),
                    enabled = enabled,
                )
                KIND_SELECT -> SelectControl(flag, enabled, actions)
                else -> Unit
            }
        }
        Text(flag.summary, style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(flag.ident, style = CorveneTheme.textStyles.codeSmall, color = colors.textTertiary)
            if (flag.nature == "Bug fix") Label(stringResource(R.string.set_flags_bug_fix))
            if (flag.restart) Label(stringResource(R.string.set_flags_needs_restart))
        }
        when (flag.kind) {
            KIND_NUMBER -> NumberControl(flag, enabled, actions)
            KIND_TEXT -> TextControl(flag, enabled, actions)
            else -> Unit
        }
        if (error != null) {
            Text(
                error,
                Modifier.testTag("$TAG_FLAG_ERROR${flag.slug}"),
                style = MaterialTheme.typography.bodySmall,
                color = colors.danger.fg,
            )
        }
        if (expanded) {
            if (flag.ghdBehaviour.isNotBlank()) {
                Text(
                    stringResource(R.string.set_flags_ghd, flag.ghdBehaviour),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textSecondary,
                )
            }
            Text(
                stringResource(R.string.set_flags_value_ghd, flag.optionLabel(flag.ghdValue)),
                style = MaterialTheme.typography.bodySmall,
                color = colors.textSecondary,
            )
        }
    }
}

private fun FlagVm.optionLabel(value: String): String = options.firstOrNull { it.value == value }?.label ?: value

/** A select flag: a chip with the current option that opens the options menu. */
@Composable
private fun SelectControl(flag: FlagVm, enabled: Boolean, actions: FlagActions) {
    var open by remember { mutableStateOf(false) }
    Box {
        PrimerChip(
            flag.valueLabel,
            selected = false,
            onClick = { open = true },
            dropdown = true,
            enabled = enabled,
            modifier = Modifier.testTag("$TAG_FLAG_CONTROL${flag.slug}"),
        )
        ActionMenu(expanded = open, onDismissRequest = { open = false }) {
            flag.options.forEach { option ->
                ActionMenuItem(
                    option.label,
                    {
                        open = false
                        if (option.value != flag.value) actions.set(flag.slug, option.value)
                    },
                    checked = option.value == flag.value,
                    modifier = Modifier.testTag("$TAG_FLAG_OPTION${flag.slug}:${option.value}"),
                )
            }
        }
    }
}

/** A number flag: − value unit +, and a slider over min…max when the range is short enough to drag through. */
@Composable
private fun NumberControl(flag: FlagVm, enabled: Boolean, actions: FlagActions) {
    val min = flag.min ?: 0L
    val max = flag.max ?: Long.MAX_VALUE
    val value = flag.value.toLongOrNull() ?: min
    val colors = CorveneTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        PrimerIconButton(
            Octicons.Dash,
            stringResource(R.string.set_flags_decrease),
            { actions.set(flag.slug, (value - 1).coerceIn(min, max).toString()) },
            Modifier.size(40.dp).testTag("$TAG_FLAG_DECREASE${flag.slug}"),
            enabled = enabled && value > min,
        )
        Text(
            listOfNotNull(value.toString(), flag.unit).joinToString(" "),
            Modifier.testTag("$TAG_FLAG_VALUE${flag.slug}"),
            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
            color = colors.textPrimary,
        )
        PrimerIconButton(
            Octicons.Plus,
            stringResource(R.string.set_flags_increase),
            { actions.set(flag.slug, (value + 1).coerceIn(min, max).toString()) },
            Modifier.size(40.dp).testTag("$TAG_FLAG_INCREASE${flag.slug}"),
            enabled = enabled && value < max,
        )
    }
    val range = max - min
    if (flag.max != null && range in 1..SLIDER_MAX_STEPS) {
        var dragged by remember(value) { mutableFloatStateOf(value.toFloat()) }
        Slider(
            value = dragged,
            onValueChange = { dragged = it },
            onValueChangeFinished = {
                val picked = dragged.toLong().coerceIn(min, max)
                if (picked != value) actions.set(flag.slug, picked.toString())
            },
            valueRange = min.toFloat()..max.toFloat(),
            steps = (range - 1).toInt().coerceAtLeast(0),
            enabled = enabled,
            colors = SliderDefaults.colors(thumbColor = colors.accent.emphasis, activeTrackColor = colors.accent.emphasis),
        )
    }
}

/** A text flag: the field and Apply; the engine's reason shows under it when the value is refused. */
@Composable
private fun TextControl(flag: FlagVm, enabled: Boolean, actions: FlagActions) {
    var text by rememberSaveable(flag.slug, flag.value) { mutableStateOf(flag.value) }
    val apply = { if (text != flag.value) actions.set(flag.slug, text) }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        PrimerTextField(
            text,
            { text = it },
            Modifier.weight(1f).testTag("$TAG_FLAG_CONTROL${flag.slug}"),
            enabled = enabled,
            monospace = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, imeAction = ImeAction.Done),
        )
        PrimerButton(
            stringResource(R.string.set_flags_apply_text),
            apply,
            Modifier.testTag("$TAG_FLAG_APPLY${flag.slug}"),
            enabled = enabled && text != flag.value,
        )
    }
}

private const val KIND_TOGGLE = "toggle"
private const val KIND_SELECT = "select"
private const val KIND_NUMBER = "number"
private const val KIND_TEXT = "text"
private const val SLIDER_MAX_STEPS = 100L
private const val DISABLED_ALPHA = 0.5f

const val TAG_FLAGS = "set_flags"
const val TAG_FLAGS_RESTART = "set_flags_restart"
const val TAG_FLAGS_RELAUNCH = "set_flags_relaunch"
const val TAG_FLAGS_PRESET = "set_flags_preset"
const val TAG_FLAGS_PRESET_ITEM = "set_flags_preset_"
const val TAG_FLAGS_RESET_ALL = "set_flags_reset_all"
const val TAG_FLAGS_CONFIRM = "set_flags_confirm"
const val TAG_FLAGS_QUERY = "set_flags_query"
const val TAG_FLAGS_STATE = "set_flags_state"
const val TAG_FLAGS_BUG_FIXES = "set_flags_bug_fixes"
const val TAG_FLAG = "set_flag_"
const val TAG_FLAG_DOT = "set_flag_dot_"
const val TAG_FLAG_RESET = "set_flag_reset_"
const val TAG_FLAG_CONTROL = "set_flag_control_"
const val TAG_FLAG_OPTION = "set_flag_option_"
const val TAG_FLAG_DECREASE = "set_flag_decrease_"
const val TAG_FLAG_INCREASE = "set_flag_increase_"
const val TAG_FLAG_VALUE = "set_flag_value_"
const val TAG_FLAG_APPLY = "set_flag_apply_"
const val TAG_FLAG_ERROR = "set_flag_error_"
