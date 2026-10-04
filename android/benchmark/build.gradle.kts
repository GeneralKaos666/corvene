import com.wasimaster.corvene.buildlogic.AndroidConfig
import com.wasimaster.corvene.buildlogic.libs

// Macrobenchmarks and the baseline profile generator, run on a connected
// phone against :app's non-debuggable variants. Included only with
// -Pcorvene.benchmark=true (settings.gradle.kts):
//   ./gradlew :app:generateFossReleaseBaselineProfile -Pcorvene.benchmark=true -Pcorvene.abis=arm64-v8a
//   ./gradlew :benchmark:connectedFossBenchmarkReleaseAndroidTest -Pcorvene.benchmark=true -Pcorvene.abis=arm64-v8a
// The journeys open the first repository of the list; on a fresh install
// they add /sdcard/Corvene/bench (instrumentation argument corvene.benchRepo)
// through x-corvene://openLocalRepo, which needs All files access.

plugins {
    id("com.android.test")
    id("corvene.kotlin-options")
    id("androidx.baselineprofile")
}

android {
    namespace = "com.wasimaster.corvene.benchmark"
    compileSdk {
        version = release(AndroidConfig.COMPILE_SDK) {
            minorApiLevel = AndroidConfig.COMPILE_SDK_MINOR
        }
    }
    defaultConfig {
        // baseline profiles need 28+ (33+ unrooted)
        minSdk = 28
        targetSdk = AndroidConfig.TARGET_SDK
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    targetProjectPath = ":app"
    flavorDimensions += AndroidConfig.DIMENSION
    productFlavors {
        AndroidConfig.FLAVOURS.forEach { flavour -> create(flavour) { dimension = AndroidConfig.DIMENSION } }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    experimentalProperties["android.experimental.self-instrumenting"] = true
}

baselineProfile {
    useConnectedDevices = true
}

dependencies {
    implementation(libs.androidx.benchmark.macro.junit4)
    implementation(libs.androidx.uiautomator)
    implementation(libs.androidx.test.junit)
    implementation(libs.junit)
}
