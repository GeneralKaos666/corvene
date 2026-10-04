package com.wasimaster.corvene.settings

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.rememberAvatarPath

/** Settings › Accounts wired to the engine: `session().accounts`, `signOut(endpoint)`; sign-in is [onSignIn]'s screen. */
@Composable
fun AccountsRoute(onSignIn: (enterprise: Boolean) -> Unit, modifier: Modifier = Modifier, contentPadding: PaddingValues = PaddingValues()) {
    val core = LocalCore.current
    val session by rememberCoreQuery { session() }
    val accounts = session.value?.accounts ?: return
    val avatars = accounts.associate { account ->
        key(account.endpoint) {
            val path by rememberAvatarPath(account.avatarUrl)
            account.endpoint to path
        }
    }
    AccountsScreen(
        accounts,
        avatars,
        onSignIn = onSignIn,
        onSignOut = { account -> core.dispatch { signOut(account.endpoint) } },
        modifier = modifier,
        contentPadding = contentPadding,
    )
}
