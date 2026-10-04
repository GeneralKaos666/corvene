package com.wasimaster.corvene.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ColorMode
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.DesignStyle
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.RadioRow
import com.wasimaster.corvene.design.StyleMiniature
import com.wasimaster.corvene.ffi.gen.DesignStyleVm
import com.wasimaster.corvene.ffi.gen.SettingsVm
import com.wasimaster.corvene.ffi.gen.ThemeVm

/**
 * Settings › Appearance: the three design styles as preview cards (each
 * drawn in its own style, light or dark like the app is now), then the theme.
 * A style pinned by flag `113-design-style` shows what is drawn and keeps the
 * stored choice for later.
 */
@Composable
fun AppearanceScreen(
    settings: SettingsVm,
    dark: Boolean,
    onStyle: (DesignStyleVm) -> Unit,
    onTheme: (ThemeVm) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    Column(
        modifier
            .fillMaxSize()
            .background(CorveneTheme.colors.bgCanvas)
            .verticalScroll(rememberScrollState())
            .padding(contentPadding)
            .padding(bottom = 24.dp),
    ) {
        ActionListGroupHeader(stringResource(R.string.set_style))
        if (settings.designStylePinned) {
            Flash(
                stringResource(R.string.set_style_pinned),
                variant = FlashVariant.Warning,
                modifier = Modifier.padding(horizontal = CorveneTheme.metrics.gutter, vertical = 4.dp),
            )
        }
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = CorveneTheme.metrics.gutter, vertical = 8.dp)
                .selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            listOf(
                Triple(DesignStyleVm.GIT_HUB_MOBILE, R.string.set_style_github_mobile, R.string.set_style_github_mobile_body),
                Triple(DesignStyleVm.GIT_HUB_DESKTOP, R.string.set_style_github_desktop, R.string.set_style_github_desktop_body),
                Triple(DesignStyleVm.MATERIAL, R.string.set_style_material, R.string.set_style_material_body),
            ).forEach { (style, title, body) ->
                StyleCard(
                    style = style,
                    title = stringResource(title),
                    body = stringResource(body),
                    selected = settings.designStyleSetting == style,
                    enabled = !settings.designStylePinned,
                    dark = dark,
                    onClick = { onStyle(style) },
                    modifier = Modifier.weight(1f).testTag("$TAG_STYLE${style.name}"),
                )
            }
        }
        ActionListGroupHeader(stringResource(R.string.set_theme), Modifier.padding(top = 8.dp))
        Column(Modifier.selectableGroup()) {
            listOf(
                Triple(ThemeVm.SYSTEM, R.string.set_theme_system, R.string.set_theme_system_body),
                Triple(ThemeVm.LIGHT, R.string.set_theme_light, null),
                Triple(ThemeVm.DARK, R.string.set_theme_dark, null),
                Triple(ThemeVm.HIGH_CONTRAST, R.string.set_theme_high_contrast, R.string.set_theme_high_contrast_body),
            ).forEach { (theme, label, caption) ->
                RadioRow(
                    stringResource(label),
                    selected = settings.theme == theme,
                    onClick = { onTheme(theme) },
                    caption = caption?.let { stringResource(it) },
                    modifier = Modifier.testTag("$TAG_THEME${theme.name}"),
                )
            }
        }
    }
}

/** One style's card: a miniature of its chrome drawn in that style, the name, a line about it. */
@Composable
private fun StyleCard(
    style: DesignStyleVm,
    title: String,
    body: String,
    selected: Boolean,
    enabled: Boolean,
    dark: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = CorveneTheme.colors
    val shape = RoundedCornerShape(CorveneTheme.metrics.cornerLarge)
    Box(
        modifier
            .clip(shape)
            .border(if (selected) 2.dp else 1.dp, if (selected) colors.accent.emphasis else colors.borderDefault, shape),
    ) {
        Column(
            Modifier.padding(8.dp).clearAndSetSemantics {},
            verticalArrangement = Arrangement.spacedBy(6.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CorveneTheme(style = style.toDesignStyle(), colorMode = if (dark) ColorMode.Dark else ColorMode.Light, dynamicColor = true) {
                StyleMiniature(stringResource(R.string.set_sample_repository), stringResource(R.string.set_sample_commit))
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                if (selected) Octicon(Octicons.CheckCircleFill, null, tint = OcticonTint.Link)
                Text(
                    title,
                    style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                    color = if (enabled) colors.textPrimary else colors.textDisabled,
                    textAlign = TextAlign.Center,
                )
            }
            Text(body, style = MaterialTheme.typography.bodySmall, color = colors.textSecondary, textAlign = TextAlign.Center, minLines = 2)
        }
        // over the miniature, whose sample button must not take the tap
        Box(
            Modifier
                .matchParentSize()
                .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
                .semantics { contentDescription = "$title. $body" },
        )
    }
}

fun DesignStyleVm.toDesignStyle(): DesignStyle = when (this) {
    DesignStyleVm.GIT_HUB_MOBILE -> DesignStyle.GitHubMobile
    DesignStyleVm.GIT_HUB_DESKTOP -> DesignStyle.GitHubDesktop
    DesignStyleVm.MATERIAL -> DesignStyle.Material
}

fun DesignStyle.toVm(): DesignStyleVm = when (this) {
    DesignStyle.GitHubMobile -> DesignStyleVm.GIT_HUB_MOBILE
    DesignStyle.GitHubDesktop -> DesignStyleVm.GIT_HUB_DESKTOP
    DesignStyle.Material -> DesignStyleVm.MATERIAL
}

const val TAG_STYLE = "set_style_"
const val TAG_THEME = "set_theme_"
