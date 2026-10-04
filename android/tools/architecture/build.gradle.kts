plugins {
    kotlin("jvm")
}

// House rules as tests, read off the sources with Konsist (ArchitectureTest).
// Part of `./gradlew unitTests` and `staticAnalysis`; alone:
//
//     ./gradlew :tools:architecture:test

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

dependencies {
    testImplementation(libs.junit)
    testImplementation(libs.konsist)
}

tasks.test {
    maxHeapSize = "2g"
    // The rules read the source tree, not this module's classes: the tree is
    // the real input, or the task would stay up to date through any change.
    inputs.files(
        fileTree(rootDir) {
            include("**/*.kt", "**/*.kts", "**/robolectric.properties")
            exclude("**/build/**", ".gradle/**", "build-logic/build/**")
        },
    ).withPathSensitivity(PathSensitivity.RELATIVE).withPropertyName("sources")
}
