// The Gradle project that packages Corvane for Android: the Rust library
// (built by cargo-ndk, see build.sh) inside a NativeActivity.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Corvane"
include(":app")
// the tree-sitter grammars as an on-demand feature module (Google Play)
include(":grammars")
