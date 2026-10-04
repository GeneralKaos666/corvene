package com.wasimaster.corvene.onboarding

import android.content.Context
import android.os.Build
import android.widget.Toast
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.ffi.Core
import com.wasimaster.corvene.ffi.LocalCore
import com.wasimaster.corvene.ffi.rememberCoreQuery
import com.wasimaster.corvene.platform.openCustomTab
import com.wasimaster.corvene.platform.rememberAvatarPath
import com.wasimaster.corvene.platform.writeClipboard

/**
 * The Welcome flow wired to the engine: `session()` for the sign-in state
 * and the accounts, `signIn` / `signInWithToken` / `cancelSignIn`,
 * `setGlobalIdentity`, then `completeWelcome` and [onFinished]. A sign-in
 * that worked moves on to Configure Git by itself (GHD).
 */
@Composable
fun WelcomeRoute(onFinished: () -> Unit, modifier: Modifier = Modifier) {
    val core = LocalCore.current
    val context = LocalContext.current
    val session by rememberCoreQuery { session() }
    var step by rememberSaveable { mutableStateOf(WelcomeStep.Start) }
    val accounts = session.value?.accounts.orEmpty()
    val account = accounts.lastOrNull()
    val avatar by rememberAvatarPath(account?.avatarUrl)
    var seen by rememberSaveable { mutableIntStateOf(-1) }
    LaunchedEffect(accounts.size, session.value != null) {
        if (session.value == null) return@LaunchedEffect
        if (seen in 0 until accounts.size && step == WelcomeStep.SignIn) step = WelcomeStep.ConfigureGit
        seen = accounts.size
    }
    val actions = remember(core, context) { CoreSignInActions(core, context) }
    WelcomeScreen(
        WelcomeState(step, session.value?.signIn, account, avatar),
        actions,
        onStep = { step = it },
        onFinish = { name, email ->
            core.dispatch {
                if (name != null && email != null) setGlobalIdentity(name, email)
                completeWelcome()
            }
            onFinished()
        },
        modifier = modifier,
        initialName = account?.name ?: account?.login.orEmpty(),
    )
}

/**
 * Sign-in on its own (Settings › Accounts): the panel under the caller's top
 * bar; [onSignedIn] once an account was added.
 */
@Composable
fun SignInRoute(
    enterprise: Boolean,
    onSignedIn: () -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(),
) {
    val core = LocalCore.current
    val context = LocalContext.current
    val session by rememberCoreQuery { session() }
    val accounts = session.value?.accounts.orEmpty()
    var seen by rememberSaveable { mutableIntStateOf(-1) }
    LaunchedEffect(accounts.size, session.value != null) {
        if (session.value == null) return@LaunchedEffect
        if (seen in 0 until accounts.size) onSignedIn()
        seen = accounts.size
    }
    val actions = remember(core, context) { CoreSignInActions(core, context) }
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(contentPadding)
            .padding(horizontal = 16.dp, vertical = 16.dp),
    ) {
        SignInPanel(session.value?.signIn, signedIn = null, actions, startWithEnterprise = enterprise)
    }
}

/** [SignInActions] as engine dispatches; pages open in a Custom Tab. */
class CoreSignInActions(private val core: Core, private val context: Context) : SignInActions {
    override fun signIn() = core.dispatch { signIn(null) }

    override fun signInEnterprise(host: String, token: String) = core.dispatch { signInWithToken(host, token) }

    override fun cancel() = core.dispatch { cancelSignIn() }

    override fun openUrl(url: String) = openCustomTab(context, url)

    override fun copy(text: String) {
        writeClipboard(context, text)
        // Android 13+ confirms a copy itself
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            Toast.makeText(context, context.getString(R.string.onb_copied), Toast.LENGTH_SHORT).show()
        }
    }
}
