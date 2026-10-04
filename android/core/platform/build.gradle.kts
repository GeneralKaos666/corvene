plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
}

// What the engine asks of Android and what Android asks of the app: pickers
// (with the SAF import), URLs and Custom Tabs, the clipboard, toasts,
// notifications, the transfer service, the documents provider, the hourly
// fetch worker, Termux, opening files in other apps.
android {
    namespace = "com.wasimaster.corvene.platform"
    resourcePrefix = "plt_"
}

dependencies {
    implementation(project(":core:common"))
    implementation(project(":core:ffi"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.browser)
    implementation(libs.androidx.work.runtime)
    implementation(libs.kotlinx.coroutines.android)
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.junit)
    testImplementation(libs.androidx.work.testing)
    testImplementation(libs.kotlinx.coroutines.test)
}
