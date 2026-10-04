import com.wasimaster.corvene.buildlogic.libs

// A feature module (`:feature:*`): a Compose library over the core modules.
// Screens take view-model records and lambdas, never the engine; the route
// composable next to each screen does the querying (Konsist checks this).

plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
    id("corvene.screenshots")
}

dependencies {
    "implementation"(project(":core:common"))
    "implementation"(project(":core:design"))
    "implementation"(project(":core:ffi"))
    "implementation"(project(":core:platform"))
    "implementation"(libs.androidx.lifecycle.runtime.compose)
    "implementation"(libs.kotlinx.coroutines.android)
    "testImplementation"(libs.kotlinx.coroutines.test)
    "testImplementation"(libs.turbine)
}
