plugins {
    id("corvene.android.feature")
}

// The Welcome flow (GHD's Welcome: Start → Sign in → Configure Git) and the
// sign-in panel Settings › Accounts reuses: device code or browser sign-in
// for GitHub.com, a token for GitHub Enterprise.
android {
    namespace = "com.wasimaster.corvene.onboarding"
    resourcePrefix = "onb_"
}
