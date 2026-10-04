plugins {
    id("corvene.android.feature")
}

// Settings (GHD's Preferences): the sections list and every section
// (Accounts, Integrations, Git, Appearance, Notifications, Prompts, Advanced,
// Accessibility, About), the Flags screen and Repository settings. Medium and
// expanded widths show the list and the section side by side.
android {
    namespace = "com.wasimaster.corvene.settings"
    resourcePrefix = "set_"
}

dependencies {
    implementation(libs.androidx.compose.adaptive)
    implementation(libs.androidx.compose.adaptive.layout)
}
