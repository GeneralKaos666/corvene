plugins {
    `kotlin-dsl`
}

dependencies {
    implementation(libs.gradle.android)
    implementation(libs.gradle.kotlin)
    implementation(libs.gradle.kotlin.compose)
    implementation(libs.gradle.kotlin.serialization)
    implementation(libs.gradle.detekt)
    implementation(libs.gradle.roborazzi)
    implementation(libs.gradle.kover)
    implementation(libs.gradle.licensee)
    implementation(libs.gradle.dependency.analysis)
    // The precompiled plugins read the catalog through `libs` (VersionCatalogs.kt).
    implementation(files(libs.javaClass.superclass.protectionDomain.codeSource.location))
    constraints {
        // Dependency Analysis 3.19 brings a Kotlin 2.4 BOM, which drags the
        // Kotlin daemon client up while the compiler stays 2.2: every compile
        // then falls back to a separate process. Hold it to the compiler's.
        implementation(libs.gradle.kotlin.daemon.client) {
            version { strictly(libs.versions.kotlin.get()) }
            because("the compiler plugin talks to a daemon of its own version")
        }
    }
}
