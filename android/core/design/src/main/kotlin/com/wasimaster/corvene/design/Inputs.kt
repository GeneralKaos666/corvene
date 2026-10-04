package com.wasimaster.corvene.design

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.selection.triStateToggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Primer's TextInput / Textarea: a [label] above (required by Primer; pass
 * null only when a visible heading names the field), the field, an optional
 * [caption] below. [monospace] for commit messages and paths. Material draws
 * M3's outlined field instead.
 */
@Composable
fun PrimerTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    placeholder: String? = null,
    caption: String? = null,
    singleLine: Boolean = true,
    minLines: Int = 1,
    maxLines: Int = if (singleLine) 1 else Int.MAX_VALUE,
    monospace: Boolean = false,
    enabled: Boolean = true,
    leadingIcon: OcticonIcon? = null,
    trailing: (@Composable () -> Unit)? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    password: Boolean = false,
) {
    val colors = CorveneTheme.colors
    val transformation = if (password) PasswordVisualTransformation() else VisualTransformation.None
    val keyboard = if (password) keyboardOptions.copy(keyboardType = KeyboardType.Password) else keyboardOptions
    val textStyle = if (monospace) CorveneTheme.textStyles.code else MaterialTheme.typography.bodyLarge
    if (LocalDesignStyle.current == DesignStyle.Material) {
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = modifier.fillMaxWidth(),
            enabled = enabled,
            textStyle = textStyle,
            label = label?.let { { Text(it) } },
            placeholder = placeholder?.let { { Text(it) } },
            supportingText = caption?.let { { Text(it) } },
            leadingIcon = leadingIcon?.let { { Octicon(it, null, tint = OcticonTint.Secondary) } },
            trailingIcon = trailing,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboard,
            visualTransformation = transformation,
            colors = OutlinedTextFieldDefaults.colors(),
        )
        return
    }
    var focused by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(CorveneTheme.metrics.cornerMedium)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (label != null) Text(label, style = MaterialTheme.typography.labelLarge, color = colors.textPrimary)
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            enabled = enabled,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboard,
            visualTransformation = transformation,
            textStyle = textStyle.copy(color = if (enabled) colors.textPrimary else colors.textDisabled),
            cursorBrush = SolidColor(colors.accent.fg),
            modifier = Modifier
                .fillMaxWidth()
                .onFocusChanged { focused = it.isFocused },
            decorationBox = { inner ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .defaultMinSize(minHeight = CorveneTheme.metrics.controlHeight)
                        .clip(shape)
                        .background(if (enabled) colors.bgDefault else colors.bgSubtle)
                        .border(if (focused) 2.dp else 1.dp, if (focused) colors.accent.emphasis else colors.borderDefault, shape)
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = if (singleLine) Alignment.CenterVertically else Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (leadingIcon != null) Octicon(leadingIcon, null, tint = OcticonTint.Secondary)
                    Box(Modifier.weight(1f)) {
                        if (value.isEmpty() && placeholder != null) {
                            Text(placeholder, style = textStyle, color = colors.textPlaceholder, maxLines = maxLines)
                        }
                        inner()
                    }
                    trailing?.invoke()
                }
            },
        )
        if (caption != null) Text(caption, style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
    }
}

/**
 * The filter input of lists and [SelectPanel]s: a search icon, the
 * [placeholder] ("Filter"), and a clear button once something is typed.
 */
@Composable
fun FilterField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    clearDescription: String = "",
) {
    PrimerTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        placeholder = placeholder,
        leadingIcon = Octicons.Search,
        trailing = if (value.isEmpty()) {
            null
        } else {
            { PrimerIconButton(Octicons.X, clearDescription, { onValueChange("") }, Modifier.size(32.dp), tint = OcticonTint.Secondary) }
        },
    )
}

/**
 * A checkbox with three states ([ToggleableState.Indeterminate] = a file
 * partly included). [onClick] null makes it a mere indicator inside a row
 * that handles the tap; otherwise it has its own 48 dp target.
 */
@Composable
fun PrimerCheckbox(
    state: ToggleableState,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    contentDescription: String? = null,
) {
    val colors = CorveneTheme.colors
    val described = if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier
    if (LocalDesignStyle.current == DesignStyle.Material) {
        TriStateCheckbox(state, onClick, modifier.then(described), enabled, CheckboxDefaults.colors())
        return
    }
    val on = state != ToggleableState.Off
    val shape = RoundedCornerShape(if (LocalDesignStyle.current == DesignStyle.GitHubDesktop) 3.dp else 4.dp)
    val alpha = if (enabled) 1f else DISABLED
    val toggle = if (onClick != null) {
        Modifier
            .minimumInteractiveComponentSize()
            .triStateToggleable(state, enabled = enabled, role = Role.Checkbox, onClick = onClick)
    } else {
        Modifier
    }
    Box(modifier.then(toggle).then(described), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .size(18.dp)
                .clip(shape)
                .background(if (on) colors.accent.emphasis.copy(alpha = alpha) else colors.bgDefault)
                .border(1.dp, if (on) Color.Transparent else colors.borderEmphasis.copy(alpha = alpha), shape),
            contentAlignment = Alignment.Center,
        ) {
            when (state) {
                ToggleableState.On -> OcticonColored(Octicons.Check, null, colors.textOnEmphasis, size = 14.dp)
                ToggleableState.Indeterminate -> OcticonColored(Octicons.Dash, null, colors.textOnEmphasis, size = 14.dp)
                ToggleableState.Off -> Unit
            }
        }
    }
}

/**
 * Primer's ToggleSwitch: a setting that applies at once. Wrap it in a row
 * with [Modifier.toggleable] for a labelled switch ([SwitchRow]).
 */
@Composable
fun PrimerSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val colors = CorveneTheme.colors
    val switchColors = if (LocalDesignStyle.current == DesignStyle.Material) {
        SwitchDefaults.colors()
    } else {
        SwitchDefaults.colors(
            checkedThumbColor = colors.textOnEmphasis,
            checkedTrackColor = colors.accent.emphasis,
            checkedBorderColor = colors.accent.emphasis,
            uncheckedThumbColor = colors.iconSecondary,
            uncheckedTrackColor = colors.bgSubtle,
            uncheckedBorderColor = colors.borderDefault,
        )
    }
    Switch(checked, onCheckedChange, modifier, enabled = enabled, colors = switchColors)
}

/** A labelled [PrimerSwitch]: the whole row toggles. */
@Composable
fun SwitchRow(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    caption: String? = null,
    enabled: Boolean = true,
) {
    val colors = CorveneTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .toggleable(checked, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange)
            .defaultMinSize(minHeight = CorveneTheme.metrics.rowHeight)
            .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = if (enabled) colors.textPrimary else colors.textDisabled)
            if (caption != null) Text(caption, style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
        }
        PrimerSwitch(checked, null, enabled = enabled)
    }
}

/** One option of a radio group, with its label; the whole row selects. */
@Composable
fun RadioRow(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    caption: String? = null,
    enabled: Boolean = true,
) {
    val colors = CorveneTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .defaultMinSize(minHeight = CorveneTheme.metrics.rowHeight)
            .padding(horizontal = (CorveneTheme.metrics.gutter - 12.dp).coerceAtLeast(0.dp), vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected,
            onClick = null,
            enabled = enabled,
            modifier = Modifier.minimumInteractiveComponentSize(),
            colors = if (LocalDesignStyle.current == DesignStyle.Material) {
                RadioButtonDefaults.colors()
            } else {
                RadioButtonDefaults.colors(selectedColor = colors.accent.emphasis, unselectedColor = colors.borderEmphasis)
            },
        )
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = if (enabled) colors.textPrimary else colors.textDisabled)
            if (caption != null) Text(caption, style = MaterialTheme.typography.bodySmall, color = colors.textSecondary)
        }
    }
}

private const val DISABLED = 0.5f

@Preview(widthDp = 360, heightDp = 1600)
@Composable
private fun InputsPreview() {
    DesignStyleSamples {
        PrimerTextField("Fix the diff gutter", {}, label = "Summary")
        PrimerTextField("", {}, placeholder = "Description", singleLine = false, minLines = 3)
        FilterField("", {}, placeholder = "Filter")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            PrimerCheckbox(ToggleableState.On, {})
            PrimerCheckbox(ToggleableState.Indeterminate, {})
            PrimerCheckbox(ToggleableState.Off, {})
            PrimerSwitch(checked = true, onCheckedChange = {})
            PrimerSwitch(checked = false, onCheckedChange = {})
        }
        SwitchRow("Hide whitespace", checked = true, onCheckedChange = {})
        RadioRow("System", selected = true, onClick = {})
    }
}
