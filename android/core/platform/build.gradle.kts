plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
}

// What the engine asks of Android: pickers, URLs, the clipboard, toasts (and
// from M-A3 notifications, the transfer service, the documents provider,
// Termux). Turns HostRequests into platform calls and answers back.
android {
    namespace = "com.wasimaster.corvene.platform"
    resourcePrefix = "plt_"
}

dependencies {
    implementation(project(":core:common"))
    implementation(project(":core:ffi"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.kotlinx.coroutines.android)
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.junit)
}
