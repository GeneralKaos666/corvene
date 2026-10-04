// Corvene for Android: Kotlin + Jetpack Compose over the Rust engine
// (crates/corvene-ffi). See README.md; the design is .docs/android/design-compose-app.md.

pluginManagement {
    // The convention plugins (corvene.android.*, corvene.rust, ...).
    includeBuild("build-logic")
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

plugins {
    // A JDK 17+ toolchain for machines without one. F-Droid's recipe deletes
    // this line (their builders have no network for toolchains).
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

/** A build switch: `-P<name>`, then local.properties, then the environment (`CORVENE_X_Y`), then off. */
fun flag(name: String): Boolean {
    val local = java.util.Properties()
    file("local.properties").takeIf { it.exists() }?.inputStream()?.use(local::load)
    val env = name.replace('.', '_').replace(Regex("([a-z])([A-Z])"), "$1_$2").uppercase()
    return (providers.gradleProperty(name).orNull ?: local.getProperty(name) ?: System.getenv(env) ?: "false").toBoolean()
}

rootProject.name = "Corvene"

include(":app")
include(":core:common")
include(":core:design")
include(":core:ffi")
include(":core:platform")
include(":feature:repositories")
include(":feature:changes")
include(":feature:branches")
include(":feature:history")
include(":feature:mco")
include(":feature:settings")
include(":feature:onboarding")
// Konsist house rules (a plain JVM module, tests only).
include(":tools:architecture")

// The on-demand grammar module (Google Play) comes with M-A6; the switch is
// read here already so the name stays put.
if (flag("corvene.play")) logger.info("corvene.play: the :grammars module arrives with M-A6")
// Macrobenchmarks and the baseline profile generator (a phone is needed to run them).
if (flag("corvene.benchmark")) include(":benchmark")
