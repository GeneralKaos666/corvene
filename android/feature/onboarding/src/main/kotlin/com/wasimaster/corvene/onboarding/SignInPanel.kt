package com.wasimaster.corvene.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.wasimaster.corvene.design.Avatar
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Flash
import com.wasimaster.corvene.design.FlashVariant
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.design.Spinner
import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.SignInStepVm
import com.wasimaster.corvene.ffi.gen.SignInVm

/** What the sign-in panel's buttons do. */
interface SignInActions {
    /** GitHub.com: the flow `307-sign-in-flow` picks (the device code without a client secret). */
    fun signIn()

    fun signInEnterprise(host: String, token: String)

    fun cancel()

    fun openUrl(url: String)

    fun copy(text: String)
}

/**
 * The sign-in panel (GHD's SignIn dialog and the Welcome step's body): with
 * no sign-in running, "Sign in with browser" and "Sign in to GitHub
 * Enterprise" (a host and a personal access token); while one runs, its step
 * from [signIn]: Requesting, the device code (Copy, Open github.com, a
 * spinner while GitHub waits), the browser page, Verifying, or the error with
 * Try again. [signedIn] (the newest account) shows once it worked.
 */
@Composable
fun SignInPanel(
    signIn: SignInVm?,
    signedIn: AccountVm?,
    actions: SignInActions,
    modifier: Modifier = Modifier,
    avatarPath: String? = null,
    startWithEnterprise: Boolean = false,
) {
    var enterprise by rememberSaveable { mutableStateOf(startWithEnterprise) }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        val step = signIn?.step
        when {
            step is SignInStepVm.Requesting -> Waiting(stringResource(R.string.onb_requesting, hostOf(signIn.endpoint)), actions)
            step is SignInStepVm.Verifying -> Waiting(stringResource(R.string.onb_verifying), actions)
            step is SignInStepVm.DeviceCode -> DeviceCode(step, actions)
            step is SignInStepVm.Browser -> Browser(step, actions)
            step is SignInStepVm.Error -> {
                Flash(step.message, variant = FlashVariant.Danger, title = stringResource(R.string.onb_error_title), icon = Octicons.Alert)
                PrimerButton(
                    stringResource(R.string.onb_try_again),
                    { if (enterprise) actions.cancel() else actions.signIn() },
                    Modifier.fillMaxWidth().testTag(TAG_TRY_AGAIN),
                    variant = PrimerButtonVariant.Primary,
                )
                CancelButton(actions)
            }
            signedIn != null && !enterprise -> SignedIn(signedIn, avatarPath)
            enterprise -> EnterpriseForm(onSubmit = actions::signInEnterprise, onBack = { enterprise = false })
            else -> {
                PrimerButton(
                    stringResource(R.string.onb_sign_in_browser),
                    actions::signIn,
                    Modifier.fillMaxWidth().testTag(TAG_SIGN_IN_BROWSER),
                    variant = PrimerButtonVariant.Primary,
                    leadingIcon = Octicons.MarkGithub,
                )
                PrimerButton(
                    stringResource(R.string.onb_sign_in_enterprise),
                    { enterprise = true },
                    Modifier.fillMaxWidth().testTag(TAG_SIGN_IN_ENTERPRISE),
                    leadingIcon = Octicons.Globe,
                )
            }
        }
    }
}

@Composable
private fun ColumnScope.Waiting(text: String, actions: SignInActions) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Spinner()
        Text(text, style = MaterialTheme.typography.bodyLarge, color = CorveneTheme.colors.textSecondary)
    }
    CancelButton(actions)
}

@Composable
private fun CancelButton(actions: SignInActions) {
    PrimerButton(stringResource(R.string.onb_cancel), actions::cancel, Modifier.fillMaxWidth(), variant = PrimerButtonVariant.Invisible)
}

@Composable
private fun ColumnScope.DeviceCode(step: SignInStepVm.DeviceCode, actions: SignInActions) {
    val colors = CorveneTheme.colors
    val host = hostOf(step.verificationUri)
    Text(
        stringResource(R.string.onb_device_code_title),
        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        color = colors.textPrimary,
    )
    Text(stringResource(R.string.onb_device_code_body, host), style = MaterialTheme.typography.bodyMedium, color = colors.textSecondary)
    Text(
        step.userCode,
        Modifier.fillMaxWidth().padding(vertical = 8.dp).testTag(TAG_USER_CODE),
        style = CorveneTheme.textStyles.code.copy(
            fontSize = 32.sp,
            lineHeight = 40.sp,
            fontWeight = FontWeight.SemiBold,
            letterSpacing = 2.sp,
        ),
        color = colors.textPrimary,
        textAlign = TextAlign.Center,
    )
    PrimerButton(
        stringResource(R.string.onb_copy_code),
        { actions.copy(step.userCode) },
        Modifier.fillMaxWidth().testTag(TAG_COPY_CODE),
        leadingIcon = Octicons.Copy,
    )
    PrimerButton(
        stringResource(R.string.onb_open_github, host),
        { actions.openUrl(step.verificationUri) },
        Modifier.fillMaxWidth().testTag(TAG_OPEN_GITHUB),
        variant = PrimerButtonVariant.Primary,
        leadingIcon = Octicons.LinkExternal,
    )
    Waiting(stringResource(R.string.onb_waiting), actions)
}

@Composable
private fun ColumnScope.Browser(step: SignInStepVm.Browser, actions: SignInActions) {
    val colors = CorveneTheme.colors
    Text(
        stringResource(R.string.onb_browser_title),
        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        color = colors.textPrimary,
    )
    Text(stringResource(R.string.onb_browser_body), style = MaterialTheme.typography.bodyMedium, color = colors.textSecondary)
    PrimerButton(
        stringResource(R.string.onb_open_again),
        { actions.openUrl(step.authorizeUrl) },
        Modifier.fillMaxWidth(),
        leadingIcon = Octicons.LinkExternal,
    )
    Waiting(stringResource(R.string.onb_waiting), actions)
}

@Composable
private fun SignedIn(account: AccountVm, avatarPath: String?) {
    val colors = CorveneTheme.colors
    Row(
        Modifier.fillMaxWidth().testTag(TAG_SIGNED_IN),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Avatar(account.login, size = 40.dp, path = avatarPath)
        Column {
            Text(
                stringResource(R.string.onb_signed_in, account.name ?: account.login),
                style = MaterialTheme.typography.titleMedium,
                color = colors.textPrimary,
            )
            Text(
                stringResource(R.string.onb_signed_in_on, account.host),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.textSecondary,
            )
        }
    }
}

@Composable
private fun ColumnScope.EnterpriseForm(onSubmit: (String, String) -> Unit, onBack: () -> Unit) {
    var host by rememberSaveable { mutableStateOf("") }
    var token by rememberSaveable { mutableStateOf("") }
    Text(
        stringResource(R.string.onb_enterprise_title),
        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        color = CorveneTheme.colors.textPrimary,
    )
    PrimerTextField(
        host,
        { host = it.trim() },
        Modifier.testTag(TAG_ENTERPRISE_HOST),
        label = stringResource(R.string.onb_enterprise_host),
        placeholder = stringResource(R.string.onb_enterprise_host_hint),
    )
    PrimerTextField(
        token,
        { token = it.trim() },
        Modifier.testTag(TAG_ENTERPRISE_TOKEN),
        label = stringResource(R.string.onb_enterprise_token),
        caption = stringResource(R.string.onb_enterprise_token_caption),
        password = true,
    )
    PrimerButton(
        stringResource(R.string.onb_enterprise_sign_in),
        { onSubmit(host, token) },
        Modifier.fillMaxWidth().testTag(TAG_ENTERPRISE_SUBMIT),
        variant = PrimerButtonVariant.Primary,
        enabled = host.isNotBlank() && token.isNotBlank(),
    )
    PrimerButton(stringResource(R.string.onb_back), onBack, Modifier.fillMaxWidth(), variant = PrimerButtonVariant.Invisible)
}

/** `https://api.github.com` → `github.com`; `https://github.com/login/device` → `github.com`. */
internal fun hostOf(url: String): String =
    url.substringAfter("://").substringBefore('/').removePrefix("api.").ifEmpty { url }

const val TAG_SIGN_IN_BROWSER = "onb_sign_in_browser"
const val TAG_SIGN_IN_ENTERPRISE = "onb_sign_in_enterprise"
const val TAG_USER_CODE = "onb_user_code"
const val TAG_COPY_CODE = "onb_copy_code"
const val TAG_OPEN_GITHUB = "onb_open_github"
const val TAG_TRY_AGAIN = "onb_try_again"
const val TAG_SIGNED_IN = "onb_signed_in"
const val TAG_ENTERPRISE_HOST = "onb_enterprise_host"
const val TAG_ENTERPRISE_TOKEN = "onb_enterprise_token"
const val TAG_ENTERPRISE_SUBMIT = "onb_enterprise_submit"
