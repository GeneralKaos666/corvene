import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

// Kotlin compiler options of every Android module (AGP 9's built-in Kotlin):
// JVM 17, the extra checkers, and `-PwarningsAsErrors=true` for CI.
// Set on the compile tasks rather than the `kotlin {}` extension, whose type
// differs between built-in Kotlin and the kotlin-android plugin.

val warningsAsErrors = providers.gradleProperty("warningsAsErrors").map(String::toBoolean).orElse(false)

tasks.withType<KotlinCompile>().configureEach {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
        extraWarnings.set(true)
        allWarningsAsErrors.set(warningsAsErrors)
        freeCompilerArgs.add("-Xreport-all-warnings")
    }
}
