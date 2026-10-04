import com.android.build.api.dsl.LibraryExtension
import com.wasimaster.corvene.buildlogic.AndroidConfig
import com.wasimaster.corvene.buildlogic.libs

// An Android library module: compileSdk 36.1, minSdk 26, the `distribution`
// flavours (foss / play) every module carries so variants line up, Android
// Lint with the project's severity map and Slack's Compose checks,
// type-resolved detekt, coverage and dependency analysis.
//
// A module names its namespace and its resource prefix:
//
//     android { namespace = "com.wasimaster.corvene.design"; resourcePrefix = "cvd_" }

plugins {
    id("com.android.library")
    id("corvene.kotlin-options")
    id("corvene.detekt")
    id("org.jetbrains.kotlinx.kover")
    id("com.autonomousapps.dependency-analysis")
}

extensions.configure<LibraryExtension> {
    compileSdk {
        version = release(AndroidConfig.COMPILE_SDK) {
            minorApiLevel = AndroidConfig.COMPILE_SDK_MINOR
        }
    }
    defaultConfig {
        minSdk = AndroidConfig.MIN_SDK
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        val rules = project.file("consumer-rules.pro")
        if (rules.isFile) consumerProguardFiles(rules)
    }
    flavorDimensions += AndroidConfig.DIMENSION
    productFlavors {
        AndroidConfig.FLAVOURS.forEach { create(it) { dimension = AndroidConfig.DIMENSION } }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        buildConfig = false
        resValues = false
    }
    testOptions {
        unitTests.isIncludeAndroidResources = true
        unitTests.all { it.maxHeapSize = "2g" }
    }
    lint {
        lintConfig = rootProject.file("config/lint/lint.xml")
        abortOnError = true
        checkDependencies = false
        // the sources of the generated bindings are not ours to lint
        ignoreTestSources = false
    }
}

kover {
    currentProject {
        createVariant("unit") { add("fossDebug") }
    }
}

dependencies {
    "lintChecks"(libs.compose.lint.checks)
}
