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
include(":feature:settings")
// Konsist house rules (a plain JVM module, tests only).
include(":tools:architecture")

// The on-demand grammar module (Google Play) and the macrobenchmarks come with
// M-A6 and M-A5; the switches are read here already so the names stay put.
if (flag("corvene.play")) logger.info("corvene.play: the :grammars module arrives with M-A6")
if (flag("corvene.benchmark")) logger.info("corvene.benchmark: the :benchmark module arrives with M-A5")
