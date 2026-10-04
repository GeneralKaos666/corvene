plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
    id("corvene.rust")
}

// The engine: libcorvene_ffi.so built by cargo-ndk, its UniFFI bindings
// (com.wasimaster.corvene.ffi.gen, generated into build/generated/uniffi),
// the bundled git, and the Kotlin side of the bridge (Core, rememberCoreQuery).
// The only module that may touch JNA or the generated bindings' internals.
android {
    namespace = "com.wasimaster.corvene.ffi"
    resourcePrefix = "ffi_"
}

dependencies {
    implementation(project(":core:common"))
    // the bindings' types (RepoListVm, HostEvents, ...) are this module's API
    api(libs.kotlinx.coroutines.android)
    api(libs.jna) { artifact { type = "aar" } }
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
}
