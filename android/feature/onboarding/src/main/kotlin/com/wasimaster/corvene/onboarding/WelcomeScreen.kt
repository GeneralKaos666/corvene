package com.wasimaster.corvene.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.wasimaster.corvene.design.CorveneTheme
import com.wasimaster.corvene.design.Octicon
import com.wasimaster.corvene.design.OcticonTint
import com.wasimaster.corvene.design.Octicons
import com.wasimaster.corvene.design.PrimerButton
import com.wasimaster.corvene.design.PrimerButtonVariant
import com.wasimaster.corvene.design.PrimerTextField
import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.SignInVm

/** GHD's Welcome steps (`WelcomeStep`): Start → Sign in → Configure Git. */
enum class WelcomeStep { Start, SignIn, ConfigureGit }

/** What the Welcome flow shows besides its step: the engine's sign-in state. */
data class WelcomeState(
    val step: WelcomeStep,
    val signIn: SignInVm?,
    val account: AccountVm?,
    val avatarPath: String? = null,
)

/**
 * The Welcome flow, one step at a time (GHD `Welcome`): Start says hello;
 * Sign in is [SignInPanel] with Skip / Continue; Configure Git asks for the
 * name and email commits are made with ([initialName], [initialEmail]
 * prefilled), Finish writes them, Skip leaves git's configuration alone.
 */
@Composable
fun WelcomeScreen(
    state: WelcomeState,
    signInActions: SignInActions,
    onStep: (WelcomeStep) -> Unit,
    onFinish: (name: String?, email: String?) -> Unit,
    modifier: Modifier = Modifier,
    initialName: String = "",
    initialEmail: String = "",
) {
    Box(
        modifier
            .fillMaxSize()
            .background(CorveneTheme.colors.bgCanvas)
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .imePadding(),
        contentAlignment = Alignment.TopCenter,
    ) {
        AnimatedContent(
            state.step,
            transitionSpec = { fadeIn() togetherWith fadeOut() },
            label = "welcome",
            modifier = Modifier.widthIn(max = 560.dp),
        ) { step ->
            Column(
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 24.dp, vertical = 32.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                when (step) {
                    WelcomeStep.Start -> Start(onNext = { onStep(WelcomeStep.SignIn) })
                    WelcomeStep.SignIn -> SignInStep(state, signInActions, onStep)
                    WelcomeStep.ConfigureGit -> ConfigureGit(initialName, initialEmail, onBack = { onStep(WelcomeStep.SignIn) }, onFinish)
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.Start(onNext: () -> Unit) {
    val colors = CorveneTheme.colors
    Spacer(Modifier.height(32.dp))
    Box(
        Modifier
            .size(72.dp)
            .clip(CircleShape)
            .background(colors.bgSubtle)
            .align(Alignment.CenterHorizontally),
        contentAlignment = Alignment.Center,
    ) {
        Octicon(Octicons.MarkGithub, null, tint = OcticonTint.Primary, size = 40.dp)
    }
    Text(
        stringResource(R.string.onb_welcome_title),
        Modifier.align(Alignment.CenterHorizontally),
        style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold),
        color = colors.textPrimary,
    )
    Text(
        stringResource(R.string.onb_welcome_body),
        style = MaterialTheme.typography.bodyLarge,
        color = colors.textSecondary,
    )
    Spacer(Modifier.height(8.dp))
    PrimerButton(
        stringResource(R.string.onb_get_started),
        onNext,
        Modifier.fillMaxWidth().testTag(TAG_GET_STARTED),
        variant = PrimerButtonVariant.Primary,
    )
    StepDots(0)
}

@Composable
private fun ColumnScope.SignInStep(state: WelcomeState, actions: SignInActions, onStep: (WelcomeStep) -> Unit) {
    Header(stringResource(R.string.onb_sign_in_title), stringResource(R.string.onb_sign_in_body))
    SignInPanel(state.signIn, state.account, actions, avatarPath = state.avatarPath)
    if (state.signIn == null) {
        if (state.account != null) {
            PrimerButton(
                stringResource(R.string.onb_continue),
                { onStep(WelcomeStep.ConfigureGit) },
                Modifier.fillMaxWidth().testTag(TAG_CONTINUE),
                variant = PrimerButtonVariant.Primary,
            )
        } else {
            PrimerButton(
                stringResource(R.string.onb_skip),
                { onStep(WelcomeStep.ConfigureGit) },
                Modifier.fillMaxWidth().testTag(TAG_SKIP),
                variant = PrimerButtonVariant.Invisible,
            )
        }
    }
    StepDots(1)
}

@Composable
private fun ColumnScope.ConfigureGit(initialName: String, initialEmail: String, onBack: () -> Unit, onFinish: (String?, String?) -> Unit) {
    var name by rememberSaveable(initialName) { mutableStateOf(initialName) }
    var email by rememberSaveable(initialEmail) { mutableStateOf(initialEmail) }
    Header(stringResource(R.string.onb_configure_title), stringResource(R.string.onb_configure_body))
    PrimerTextField(name, { name = it }, Modifier.testTag(TAG_NAME), label = stringResource(R.string.onb_name))
    PrimerTextField(email, { email = it.trim() }, Modifier.testTag(TAG_EMAIL), label = stringResource(R.string.onb_email))
    PrimerButton(
        stringResource(R.string.onb_finish),
        { onFinish(name.trim(), email) },
        Modifier.fillMaxWidth().testTag(TAG_FINISH),
        variant = PrimerButtonVariant.Primary,
        enabled = name.isNotBlank() && '@' in email,
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        PrimerButton(stringResource(R.string.onb_back), onBack, Modifier.weight(1f), variant = PrimerButtonVariant.Invisible)
        PrimerButton(
            stringResource(R.string.onb_skip),
            { onFinish(null, null) },
            Modifier.weight(1f).testTag(TAG_SKIP),
            variant = PrimerButtonVariant.Invisible,
        )
    }
    StepDots(2)
}

@Composable
private fun Header(title: String, body: String) {
    val colors = CorveneTheme.colors
    Text(title, style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold), color = colors.textPrimary)
    Text(body, style = MaterialTheme.typography.bodyMedium, color = colors.textSecondary)
}

/** Three dots, the current one filled: where the pager is. */
@Composable
private fun ColumnScope.StepDots(current: Int) {
    val colors = CorveneTheme.colors
    val label = stringResource(R.string.onb_step, current + 1, WelcomeStep.entries.size)
    Row(
        Modifier.align(Alignment.CenterHorizontally).padding(top = 8.dp).testTag(label),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        WelcomeStep.entries.forEachIndexed { index, _ ->
            Box(
                Modifier
                    .size(8.dp)
                    .clip(CircleShape)
                    .background(if (index == current) colors.accent.emphasis else colors.borderDefault),
            )
        }
    }
}

const val TAG_GET_STARTED = "onb_get_started"
const val TAG_CONTINUE = "onb_continue"
const val TAG_SKIP = "onb_skip"
const val TAG_NAME = "onb_name"
const val TAG_EMAIL = "onb_email"
const val TAG_FINISH = "onb_finish"
