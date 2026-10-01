// The native library is compiled outside Gradle (packaging/android/build.sh
// runs cargo-ndk) and picked up from src/main/jniLibs.

plugins {
    id("com.android.application")
}

// the workspace version in the root Cargo.toml
val cargoVersion: String = Regex("""(?m)^version = "([^"]+)"""")
    .find(rootProject.file("../../Cargo.toml").readText())!!
    .groupValues[1]

android {
    namespace = "com.wasimaster.corvane"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.wasimaster.corvane"
        // Android 8: Vulkan 1.0 and the APIs the platform layer uses
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = cargoVersion

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        jniLibs {
            // Extract the native libraries when the app is installed: git and
            // its helpers ship as executables named lib*.so, which can only
            // run from the extracted library directory.
            useLegacyPackaging = true
            // cargo strips release builds; keep debug symbols of debug ones
            keepDebugSymbols += "**/*.so"
        }
    }

    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}
