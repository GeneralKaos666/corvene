package com.wasimaster.corvene.repositories

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.ActionMenu
import com.wasimaster.corvene.design.ActionMenuItem
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.SwitchRow

/**
 * GHD's Add Local Repository, full screen: the folder ([path], null until
 * one is picked; Choose… opens the system picker, which imports a copy of a
 * repository Corvene cannot use in place) and Add. Where "All files access"
 * can be asked for and is not granted, a note offers it ([onAllFilesAccess]).
 */
@Composable
fun AddRepositoryScreen(
    path: String?,
    onChoose: () -> Unit,
    onAdd: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    onAllFilesAccess: (() -> Unit)? = null,
) {
    RepositoryForm(
        title = stringResource(R.string.repo_add_title),
        confirm = stringResource(R.string.repo_add_confirm),
        confirmEnabled = path != null,
        onConfirm = onAdd,
        onClose = onClose,
        modifier = modifier,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            LocalPathRow(path ?: stringResource(R.string.repo_no_folder), onChoose, label = stringResource(R.string.repo_folder))
            FormCaption(stringResource(R.string.repo_add_caption))
            if (onAllFilesAccess != null) {
                FormFields {
                    Flash(
                        stringResource(R.string.repo_all_files_body),
                        title = stringResource(R.string.repo_all_files_title),
                        icon = Octicons.Info,
                        action = { PrimerButton(stringResource(R.string.repo_all_files_allow), onAllFilesAccess) },
                    )
                }
            }
        }
    }
}

/** The Create form's values (GHD `CreateRepository`). */
@Immutable
data class CreateForm(
    val name: String = "",
    val description: String = "",
    val readme: Boolean = true,
    val gitignore: String? = null,
    val license: String? = null,
)

/**
 * GHD's Create a New Repository, full screen: name, description, the folder
 * it goes in ([base]; the repository is `<base>/<name>`), "Initialize with a
 * README", a .gitignore template and a license (each a menu of the common
 * ones). Create is the top bar's action.
 */
@Composable
fun CreateRepositoryScreen(
    form: CreateForm,
    base: String,
    onChange: (CreateForm) -> Unit,
    onChooseBase: () -> Unit,
    onCreate: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val name = form.name.trim()
    RepositoryForm(
        title = stringResource(R.string.repo_create_title),
        confirm = stringResource(R.string.repo_create_confirm),
        confirmEnabled = name.isNotEmpty(),
        onConfirm = onCreate,
        onClose = onClose,
        modifier = modifier,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            FormFields {
                PrimerTextField(
                    form.name,
                    { onChange(form.copy(name = it)) },
                    Modifier.testTag(TAG_CREATE_NAME),
                    label = stringResource(R.string.repo_name),
                    caption = sanitizedName(name).takeIf { name.isNotEmpty() && it != name }
                        ?.let { stringResource(R.string.repo_name_sanitized, it) },
                )
                PrimerTextField(
                    form.description,
                    { onChange(form.copy(description = it)) },
                    label = stringResource(R.string.repo_description),
                )
            }
            LocalPathRow(
                if (name.isEmpty()) base else "$base/${sanitizedName(name)}",
                onChooseBase,
            )
            SwitchRow(
                stringResource(R.string.repo_readme),
                form.readme,
                { onChange(form.copy(readme = it)) },
                Modifier.testTag(TAG_CREATE_README),
            )
            TemplateRow(
                stringResource(R.string.repo_gitignore),
                form.gitignore,
                GITIGNORE_TEMPLATES,
                { onChange(form.copy(gitignore = it)) },
                Modifier.testTag(TAG_CREATE_GITIGNORE),
            )
            TemplateRow(
                stringResource(R.string.repo_license),
                form.license,
                LICENSES,
                { onChange(form.copy(license = it)) },
                Modifier.testTag(TAG_CREATE_LICENSE),
            )
        }
    }
}

@Composable
private fun TemplateRow(label: String, value: String?, options: List<String>, onPick: (String?) -> Unit, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    val none = stringResource(R.string.repo_none)
    Box {
        ActionListItem(
            title = label,
            description = value ?: none,
            onClick = { open = true },
            modifier = modifier,
            chevron = true,
            leading = { IconTile(Octicons.File, CorveneTheme.colors.accent) },
        )
        ActionMenu(expanded = open, onDismissRequest = { open = false }) {
            (listOf<String?>(null) + options).forEach { option ->
                ActionMenuItem(
                    option ?: none,
                    {
                        open = false
                        onPick(option)
                    },
                    checked = option == value,
                )
            }
        }
    }
}

/** GHD `sanitizedRepositoryName`: what is not a letter, digit, `.`, `_` or `-` becomes `-`. */
fun sanitizedName(name: String): String = name.replace(Regex("[^\\w.-]"), "-")

/** The common .gitignore templates (the engine bundles GHD's whole list: FFI-REQUESTS). */
val GITIGNORE_TEMPLATES = listOf(
    "Android", "C", "C++", "CMake", "Dart", "Dotnet", "Flutter", "Go", "Gradle", "Java", "Kotlin", "Maven",
    "Node", "Python", "Ruby", "Rust", "Swift", "TeX", "Unity", "VisualStudio", "Zig",
)

/** The common licenses, by the engine's names (choosealicense titles). */
val LICENSES = listOf(
    "MIT License",
    "Apache License 2.0",
    "GNU General Public License v3.0",
    "GNU General Public License v2.0",
    "GNU Affero General Public License v3.0",
    "GNU Lesser General Public License v3.0",
    "Mozilla Public License 2.0",
    "BSD 2-Clause \"Simplified\" License",
    "BSD 3-Clause \"New\" or \"Revised\" License",
    "Boost Software License 1.0",
    "Creative Commons Zero v1.0 Universal",
    "Eclipse Public License 2.0",
    "ISC License",
    "The Unlicense",
)

const val TAG_CREATE_NAME = "repo_create_name"
const val TAG_CREATE_README = "repo_create_readme"
const val TAG_CREATE_GITIGNORE = "repo_create_gitignore"
const val TAG_CREATE_LICENSE = "repo_create_license"
