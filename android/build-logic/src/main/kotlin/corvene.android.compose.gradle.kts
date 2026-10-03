import com.android.build.api.dsl.ApplicationExtension
import com.android.build.api.dsl.LibraryExtension
import com.wasimaster.corvene.buildlogic.libs
import org.jetbrains.kotlin.compose.compiler.gradle.ComposeCompilerGradlePluginExtension

// Compose for a library or the application: the compiler plugin, the BOM and
// the everyday artifacts, the project's stability file, and on demand the
// compiler's reports:
//
//   ./gradlew :feature:repositories:compileFossDebugKotlin --rerun -PcomposeMetrics=true
//
// (build/compose/{reports,metrics}; `--rerun` because the destinations are not inputs).

plugins {
    id("org.jetbrains.kotlin.plugin.compose")
}

pluginManager.withPlugin("com.android.library") {
    extensions.configure<LibraryExtension> { buildFeatures.compose = true }
}
pluginManager.withPlugin("com.android.application") {
    extensions.configure<ApplicationExtension> { buildFeatures.compose = true }
}

extensions.configure<ComposeCompilerGradlePluginExtension> {
    stabilityConfigurationFiles.add(rootProject.layout.projectDirectory.file("config/compose/stability.conf"))
    if (providers.gradleProperty("composeMetrics").map { it.isEmpty() || it.toBoolean() }.getOrElse(false)) {
        metricsDestination.set(layout.buildDirectory.dir("compose/metrics"))
        reportsDestination.set(layout.buildDirectory.dir("compose/reports"))
    }
}

dependencies {
    "implementation"(platform(libs.androidx.compose.bom))
    "implementation"(libs.androidx.compose.runtime)
    "implementation"(libs.androidx.compose.ui)
    "implementation"(libs.androidx.compose.ui.graphics)
    "implementation"(libs.androidx.compose.foundation)
    "implementation"(libs.androidx.compose.material3)
    "implementation"(libs.androidx.compose.ui.tooling.preview)
    "debugImplementation"(libs.androidx.compose.ui.tooling)
    "testImplementation"(platform(libs.androidx.compose.bom))
}
