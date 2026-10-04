import com.wasimaster.corvene.buildlogic.libs
import io.github.takahirom.roborazzi.RoborazziExtension

// Roborazzi screenshot tests on Robolectric (JVM, no device):
//
//   ./gradlew :feature:repositories:recordRoborazziFossDebug    write src/test/screenshots/*.png
//   ./gradlew :feature:repositories:verifyRoborazziFossDebug    compare against them
//
// The plain unit-test run captures without comparing.

plugins {
    id("io.github.takahirom.roborazzi")
}

extensions.configure<RoborazziExtension> {
    outputDir.set(layout.projectDirectory.dir("src/test/screenshots"))
}

dependencies {
    "testImplementation"(libs.junit)
    "testImplementation"(libs.robolectric)
    "testImplementation"(libs.roborazzi)
    "testImplementation"(libs.roborazzi.compose)
    "testImplementation"(libs.roborazzi.junit.rule)
    "testImplementation"(libs.androidx.compose.ui.test.junit4)
    "testImplementation"(libs.androidx.test.junit)
    "testImplementation"(libs.androidx.test.espresso.core)
    "debugImplementation"(libs.androidx.compose.ui.test.manifest)
}
