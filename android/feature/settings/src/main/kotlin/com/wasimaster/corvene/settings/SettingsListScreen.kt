package com.wasimaster.corvene.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.OcticonIcon
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerColors
import com.wasimaster.corvene.design.SemanticColor

/**
 * GHD's Preferences tabs (Accounts · Integrations · Git · Appearance ·
 * Notifications · Prompts · Advanced · Accessibility; no Copilot) plus
 * Corvene's Flags and About. [key] is what navigation keys and the engine's
 * `Preferences` popup name.
 */
enum class SettingsSection(val key: String, val title: Int, val summary: Int, val icon: OcticonIcon) {
    Accounts("accounts", R.string.set_accounts, R.string.set_accounts_summary, Octicons.Person),
    Integrations("integrations", R.string.set_integrations, R.string.set_integrations_summary, Octicons.Terminal),
    Git("git", R.string.set_git, R.string.set_git_summary, Octicons.GitBranch),
    Appearance("appearance", R.string.set_appearance, R.string.set_appearance_summary, Octicons.Paintbrush),
    Notifications("notifications", R.string.set_notifications, R.string.set_notifications_summary, Octicons.Bell),
    Prompts("prompts", R.string.set_prompts, R.string.set_prompts_summary, Octicons.CommentDiscussion),
    Advanced("advanced", R.string.set_advanced, R.string.set_advanced_summary, Octicons.Tools),
    Accessibility("accessibility", R.string.set_accessibility, R.string.set_accessibility_summary, Octicons.Accessibility),
    Flags("flags", R.string.set_flags, R.string.set_flags_summary, Octicons.Beaker),
    About("about", R.string.set_about, R.string.set_about_summary, Octicons.Info),
    ;

    companion object {
        fun fromKey(key: String?): SettingsSection? = entries.firstOrNull { it.key.equals(key, ignoreCase = true) }
    }
}

/** The coloured tile of a section's row (GitHub Mobile's settings list). */
private fun SettingsSection.tint(colors: PrimerColors): SemanticColor = when (this) {
    SettingsSection.Accounts, SettingsSection.Git -> colors.accent
    SettingsSection.Integrations, SettingsSection.Advanced -> colors.done
    SettingsSection.Appearance, SettingsSection.Flags -> colors.sponsors
    SettingsSection.Notifications, SettingsSection.Prompts -> colors.attention
    SettingsSection.Accessibility, SettingsSection.About -> colors.success
}

/**
 * The sections list: compact widths push a section; wider ones keep this as
 * the list pane (GHD's 150 px vertical nav) with [selected] highlighted.
 */
@Composable
fun SettingsListScreen(
    onOpen: (SettingsSection) -> Unit,
    modifier: Modifier = Modifier,
    selected: SettingsSection? = null,
    contentPadding: PaddingValues = PaddingValues(),
) {
    val colors = CorveneTheme.colors
    LazyColumn(modifier.fillMaxSize().background(colors.bgCanvas).testTag(TAG_SECTIONS), contentPadding = contentPadding) {
        items(SettingsSection.entries, key = { it.key }) { section ->
            ActionListItem(
                stringResource(section.title),
                description = if (selected == null) stringResource(section.summary) else null,
                selected = section == selected,
                chevron = selected == null,
                onClick = { onOpen(section) },
                modifier = Modifier.testTag("$TAG_SECTION${section.key}"),
                leading = { IconTile(section.icon, section.tint(colors)) },
            )
            if (selected == null && (section == SettingsSection.Accessibility)) ActionListDivider()
        }
    }
}

const val TAG_SECTIONS = "set_sections"
const val TAG_SECTION = "set_section_"
