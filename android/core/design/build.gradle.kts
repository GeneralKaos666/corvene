plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
    id("corvene.screenshots")
}

// The design system: three styles (GitHub Mobile, GitHub Desktop, Material),
// Primer tokens, fonts, Octicons and the shared components. Depends on
// nothing native, so previews and screenshot tests run anywhere.
android {
    namespace = "com.wasimaster.corvene.design"
    resourcePrefix = "cvd_"
}

dependencies {
    implementation(project(":core:common"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.coil.core)
    implementation(libs.coil.compose.core)
}
