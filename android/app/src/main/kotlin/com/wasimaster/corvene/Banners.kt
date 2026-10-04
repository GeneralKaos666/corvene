package com.wasimaster.corvene

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.ffi.gen.BannerVm

/**
 * GHD's banner under the tabs (`BannerType`): merge / rebase / cherry-pick /
 * squash / reorder outcomes, "already up to date", "Deleted branch" (Undo is
 * not exposed yet) and the conflicts banner with "View conflicts". The
 * engine sends the banner's Debug form ([BannerVm.text]); [bannerFields]
 * reads its fields until a typed record arrives (FFI-REQUESTS). Dismissing
 * hides that [BannerVm.nonce] here; the engine times banners out itself.
 */
@Composable
fun BannerFlash(banner: BannerVm?, onViewConflicts: () -> Unit, modifier: Modifier = Modifier) {
    var dismissed by rememberSaveable { mutableLongStateOf(-1L) }
    val value = banner?.takeIf { it.nonce.toLong() != dismissed } ?: return
    val fields = bannerFields(value.text)
    fun f(key: String) = fields[key].orEmpty()
    val count = fields["count"]?.toIntOrNull() ?: 0
    val text = when (value.kind) {
        "SuccessfulMerge" -> stringResource(
            R.string.app_banner_merged,
            f("their_branch").ifEmpty { stringResource(R.string.app_the_branch) },
            f("our_branch"),
        )
        "SuccessfulRebase" -> stringResource(R.string.app_banner_rebased, f("target_branch"), f("base_branch"))
        "BranchAlreadyUpToDate" -> stringResource(R.string.app_banner_up_to_date, f("our_branch"), f("their_branch"))
        "SuccessfulCherryPick" -> pluralStringResource(R.plurals.app_banner_cherry_picked, count, count, f("target_branch"))
        "CherryPickUndone" -> stringResource(R.string.app_banner_cherry_pick_undone)
        "SuccessfulSquash" -> pluralStringResource(R.plurals.app_banner_squashed, count, count)
        "SquashUndone" -> stringResource(R.string.app_banner_squash_undone)
        "SuccessfulReorder" -> pluralStringResource(R.plurals.app_banner_reordered, count, count)
        "ReorderUndone" -> stringResource(R.string.app_banner_reorder_undone)
        "BranchDeleted" -> stringResource(R.string.app_banner_deleted, f("branch"))
        "BranchRestored" -> stringResource(R.string.app_banner_restored, f("branch"))
        "ConflictsFound" -> if (f("branch").isEmpty()) {
            stringResource(R.string.app_banner_conflicts_plain, f("description"))
        } else {
            stringResource(R.string.app_banner_conflicts, f("description"), f("branch"))
        }
        else -> value.text
    }
    val conflicts = value.kind == "ConflictsFound"
    Flash(
        text,
        modifier.testTag(TAG_BANNER),
        variant = if (conflicts) FlashVariant.Warning else FlashVariant.Success,
        icon = if (conflicts) Octicons.Alert else Octicons.CheckCircle,
        flush = true,
        dismissDescription = stringResource(R.string.app_banner_dismiss),
        onDismiss = { dismissed = value.nonce.toLong() },
        action = when (value.kind) {
            "ConflictsFound" -> {
                {
                    PrimerButton(
                        stringResource(R.string.app_view_conflicts),
                        onViewConflicts,
                        variant = PrimerButtonVariant.Link,
                        modifier = Modifier.testTag(TAG_BANNER_CONFLICTS),
                    )
                }
            }
            // `861-undo-delete-branch`'s Undo needs an FFI entry point
            "BranchDeleted" -> {
                { PrimerButton(stringResource(R.string.app_undo), {}, variant = PrimerButtonVariant.Link, enabled = false) }
            }
            else -> null
        },
    )
}

/**
 * The fields of a Rust Debug form (`Kind { a: "x", b: Some("y"), n: 3 }`):
 * strings (inside `Some(..)` or not) and integers; `None` fields are absent.
 */
fun bannerFields(debug: String): Map<String, String> {
    val fields = mutableMapOf<String, String>()
    STRING_FIELD.findAll(debug).forEach { match ->
        fields[match.groupValues[1]] = match.groupValues[2].replace("\\\"", "\"").replace("\\\\", "\\")
    }
    NUMBER_FIELD.findAll(debug).forEach { match -> fields.putIfAbsent(match.groupValues[1], match.groupValues[2]) }
    return fields
}

private val STRING_FIELD = Regex("""(\w+): (?:Some\()?"((?:[^"\\]|\\.)*)"""")
private val NUMBER_FIELD = Regex("""(\w+): (\d+)""")

const val TAG_BANNER = "app_banner"
const val TAG_BANNER_CONFLICTS = "app_banner_conflicts"
