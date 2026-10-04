package com.wasimaster.corvene.onboarding

import com.wasimaster.corvene.ffi.gen.AccountVm
import com.wasimaster.corvene.ffi.gen.SignInStepVm
import com.wasimaster.corvene.ffi.gen.SignInVm

internal val SampleAccount = AccountVm(
    endpoint = "https://api.github.com",
    login = "octocat",
    name = "The Octocat",
    avatarUrl = "https://avatars.githubusercontent.com/u/583231",
    host = "github.com",
)

internal val SampleDeviceCode = SignInVm(
    endpoint = "https://api.github.com",
    step = SignInStepVm.DeviceCode(userCode = "WDJB-MJHT", verificationUri = "https://github.com/login/device"),
)

/** Does nothing: previews and screenshots. */
internal object NoSignInActions : SignInActions {
    override fun signIn() = Unit

    override fun signInEnterprise(host: String, token: String) = Unit

    override fun cancel() = Unit

    override fun openUrl(url: String) = Unit

    override fun copy(text: String) = Unit
}
