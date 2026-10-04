package com.wasimaster.corvene.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.ActionListDivider
import com.wasimaster.corvene.design.ActionListGroupHeader
import com.wasimaster.corvene.design.ActionListItem
import com.wasimaster.corvene.design.Avatar
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.IconTile
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerDialog
import com.wasimaster.corvene.ffi.gen.AccountVm

/**
 * Settings › Accounts (GHD Preferences › Accounts): each signed-in account
 * with its avatar ([avatarPaths] by endpoint) and Sign out (asks first);
 * Sign in to GitHub.com when there is no GitHub.com account, Sign in to
 * GitHub Enterprise always.
 */
@Composable
fun AccountsScreen(
    accounts: List<AccountVm>,
    avatarPaths: Map<String, String?>,
    onSignIn: (enterprise: Boolean) -> Unit,
    onSignOut: (AccountVm) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    var confirming by rememberSaveable { mutableStateOf<String?>(null) }
    val dotcom = accounts.any { it.host == GITHUB_HOST }
    LazyColumn(modifier.fillMaxSize().background(CorveneTheme.colors.bgCanvas), contentPadding = contentPadding) {
        item { ActionListGroupHeader(stringResource(R.string.set_accounts_signed_in)) }
        if (accounts.isEmpty()) {
            item { ActionListItem(stringResource(R.string.set_accounts_none), enabled = false) }
        }
        items(accounts, key = { it.endpoint }) { account ->
            ActionListItem(
                title = account.name ?: account.login,
                description = "@${account.login} · ${account.host}",
                modifier = Modifier.testTag("$TAG_ACCOUNT${account.login}"),
                leading = { Avatar(account.login, size = 40.dp, path = avatarPaths[account.endpoint]) },
                trailing = {
                    PrimerButton(
                        stringResource(R.string.set_sign_out),
                        { confirming = account.endpoint },
                        Modifier.testTag("$TAG_SIGN_OUT${account.login}"),
                        variant = PrimerButtonVariant.Danger,
                    )
                },
            )
            ActionListDivider()
        }
        item { ActionListGroupHeader(stringResource(R.string.set_accounts_add)) }
        if (!dotcom) {
            item {
                ActionListItem(
                    stringResource(R.string.set_sign_in_dotcom),
                    onClick = { onSignIn(false) },
                    modifier = Modifier.testTag(TAG_SIGN_IN),
                    leading = { IconTile(Octicons.MarkGithub, CorveneTheme.colors.accent) },
                    chevron = true,
                )
            }
        }
        item {
            ActionListItem(
                stringResource(R.string.set_sign_in_enterprise),
                onClick = { onSignIn(true) },
                modifier = Modifier.testTag(TAG_SIGN_IN_ENTERPRISE),
                leading = { IconTile(Octicons.Globe, CorveneTheme.colors.done) },
                chevron = true,
            )
        }
    }
    val target = accounts.firstOrNull { it.endpoint == confirming }
    if (target != null) {
        PrimerDialog(
            title = stringResource(R.string.set_sign_out_title),
            onDismissRequest = { confirming = null },
            confirmButton = {
                PrimerButton(
                    stringResource(R.string.set_sign_out),
                    {
                        confirming = null
                        onSignOut(target)
                    },
                    Modifier.testTag(TAG_SIGN_OUT_CONFIRM),
                    variant = PrimerButtonVariant.Danger,
                )
            },
            dismissButton = {
                PrimerButton(stringResource(R.string.set_cancel), { confirming = null }, variant = PrimerButtonVariant.Invisible)
            },
        ) {
            Text(
                stringResource(R.string.set_sign_out_body, target.login, target.host),
                color = CorveneTheme.colors.textPrimary,
            )
        }
    }
}

private const val GITHUB_HOST = "github.com"
const val TAG_ACCOUNT = "set_account_"
const val TAG_SIGN_OUT = "set_sign_out_"
const val TAG_SIGN_OUT_CONFIRM = "set_sign_out_confirm"
const val TAG_SIGN_IN = "set_sign_in"
const val TAG_SIGN_IN_ENTERPRISE = "set_sign_in_enterprise"
