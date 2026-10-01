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

    buildFeatures {
        buildConfig = true
    }

    // Two ways Corvane is distributed, from the same native library:
    //
    // foss: GitHub Releases and F-Droid. May ask for "All files access" to
    //   open repositories on shared storage in place (src/foss declares the
    //   permission) and may download packs with native code.
    // play: Google Play, whose policies allow neither.
    flavorDimensions += "distribution"
    productFlavors {
        create("foss") {
            dimension = "distribution"
            buildConfigField("boolean", "DOWNLOADED_CODE", "true")
        }
        create("play") {
            dimension = "distribution"
            buildConfigField("boolean", "DOWNLOADED_CODE", "false")
        }
    }

    // The release key stays outside the repository: a keystore file and its
    // passwords from the environment (the CI secrets). Without them the
    // release package is left unsigned, to be signed with apksigner.
    val keystore = System.getenv("CORVANE_ANDROID_KEYSTORE")
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("CORVANE_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("CORVANE_ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("CORVANE_ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (keystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
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

dependencies {
    // the periodic background fetch (CorvaneFetchWorker)
    implementation("androidx.work:work-runtime:2.10.0")
}
