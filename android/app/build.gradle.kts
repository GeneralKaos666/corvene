plugins {
    id("corvene.android.application")
    id("corvene.android.compose")
    id("corvene.rust")
    id("corvene.screenshots")
    id("org.jetbrains.kotlin.plugin.serialization")
    // the baseline profile's consumer: :benchmark (-Pcorvene.benchmark=true) produces it
    id("androidx.baselineprofile")
}

// The application: Application + MainActivity, Navigation 3 over the
// feature screens, the splash screen, the engine's start-up. See README.md
// for the build properties and build-logic/ for what the plugins do.
android {
    namespace = "com.wasimaster.corvene"
}

// The startup and scrolling profile from :benchmark's BaselineProfileGenerator,
// saved under src/<variant>/generated/baselineProfiles and compiled into
// release builds (profileinstaller installs it). Generate on a connected phone:
//   ./gradlew :app:generateFossReleaseBaselineProfile -Pcorvene.benchmark=true -Pcorvene.abis=arm64-v8a
baselineProfile {
    saveInSrc = true
    automaticGenerationDuringBuild = false
    dexLayoutOptimization = true
}

dependencies {
    implementation(project(":core:common"))
    implementation(project(":core:design"))
    implementation(project(":core:ffi"))
    implementation(project(":core:platform"))
    implementation(project(":feature:repositories"))
    implementation(project(":feature:changes"))
    implementation(project(":feature:branches"))
    implementation(project(":feature:history"))
    implementation(project(":feature:mco"))
    implementation(project(":feature:settings"))
    implementation(project(":feature:onboarding"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.androidx.compose.adaptive)
    implementation(libs.androidx.compose.adaptive.layout)
    implementation(libs.androidx.compose.adaptive.navigation)
    implementation(libs.androidx.navigation3.runtime)
    implementation(libs.androidx.navigation3.ui)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.profileinstaller)
    implementation(libs.androidx.metrics.performance)
    // only with -Pcorvene.benchmark=true (settings.gradle.kts includes :benchmark then)
    if (findProject(":benchmark") != null) "baselineProfile"(project(":benchmark"))

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
}
