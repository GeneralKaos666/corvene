import com.android.build.api.artifact.SingleArtifact
import com.android.build.api.dsl.ApplicationExtension
import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import com.wasimaster.corvene.buildlogic.ApkBudgetTask
import com.wasimaster.corvene.buildlogic.NoticesTask
import com.wasimaster.corvene.buildlogic.capitalized
import com.wasimaster.corvene.buildlogic.Abis
import com.wasimaster.corvene.buildlogic.AndroidConfig
import com.wasimaster.corvene.buildlogic.buildProperty
import com.wasimaster.corvene.buildlogic.flag
import com.wasimaster.corvene.buildlogic.libs
import com.wasimaster.corvene.buildlogic.workspaceDir

// The application: id com.wasimaster.corvene (+ `-Pcorvene.idSuffix`, `.compose`
// on debug by default, so it installs beside the GPUI build), versionName from
// the workspace's Cargo.toml, versionCode from `corvene.versionCode`.
//
// Build types:
//   debug    debuggable, one ABI
//   release  R8 (shrink + optimise), every ABI, signed from CORVENE_ANDROID_KEYSTORE*
//   fast     release minus R8: not debuggable, profile-installed, one ABI. For
//            judging speed by hand without the R8 wait. Never tests keep rules.

plugins {
    id("com.android.application")
    id("corvene.kotlin-options")
    id("corvene.detekt")
    id("org.jetbrains.kotlinx.kover")
    id("com.autonomousapps.dependency-analysis")
    id("app.cash.licensee")
}

// `version = "x.y.z"` in [workspace.package] of the root Cargo.toml
val cargoVersion: String = Regex("""(?m)^version = "([^"]+)"""")
    .find(File(workspaceDir, "Cargo.toml").readText())
    ?.groupValues?.get(1) ?: error("no version in ${workspaceDir}/Cargo.toml")
val splitApks = flag("corvene.splitApks")
val idSuffix = buildProperty("corvene.idSuffix") ?: ".compose"
val keystore: String? = System.getenv("CORVENE_ANDROID_KEYSTORE")

extensions.configure<ApplicationExtension> {
    compileSdk {
        version = release(AndroidConfig.COMPILE_SDK) {
            minorApiLevel = AndroidConfig.COMPILE_SDK_MINOR
        }
    }
    defaultConfig {
        applicationId = "com.wasimaster.corvene"
        minSdk = AndroidConfig.MIN_SDK
        targetSdk = AndroidConfig.TARGET_SDK
        versionCode = (buildProperty("corvene.versionCode") ?: "1").toInt()
        versionName = cargoVersion
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    flavorDimensions += AndroidConfig.DIMENSION
    productFlavors {
        create("foss") {
            dimension = AndroidConfig.DIMENSION
            buildConfigField("boolean", "DOWNLOADED_CODE", "true")
        }
        create("play") {
            dimension = AndroidConfig.DIMENSION
            buildConfigField("boolean", "DOWNLOADED_CODE", "false")
        }
    }
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("CORVENE_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("CORVENE_ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("CORVENE_ANDROID_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        debug {
            applicationIdSuffix = idSuffix
            versionNameSuffix = "-dev"
            if (!splitApks) ndk { abiFilters += Abis.of(project, "debug") }
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            optimization { enable = true }
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
            if (!splitApks) ndk { abiFilters += Abis.of(project, "release") }
        }
        create("fast") {
            initWith(getByName("release"))
            // both switches: under AGP 9 `optimization.enable` turns R8 on by itself
            isMinifyEnabled = false
            isShrinkResources = false
            optimization { enable = false }
            isDebuggable = false
            // library modules have debug and release only
            matchingFallbacks += "release"
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
            ndk {
                abiFilters.clear()
                if (!splitApks) abiFilters += Abis.of(project, "debug")
            }
        }
    }
    if (splitApks) {
        splits {
            abi {
                isEnable = true
                reset()
                include(*Abis.of(project, "release").toTypedArray())
                isUniversalApk = true
            }
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        buildConfig = true
    }
    packaging {
        jniLibs {
            // Extracted at install: git and its helpers ship as executables
            // named lib*.so and only run from the extracted library directory.
            useLegacyPackaging = true
            // CargoNdkTask strips what should be stripped; AGP's pass would
            // also drop the symbol tables backtraces need.
            keepDebugSymbols += "**/*.so"
        }
    }
    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }
    testOptions {
        unitTests.isIncludeAndroidResources = true
        unitTests.all { it.maxHeapSize = "2g" }
    }
    lint {
        lintConfig = rootProject.file("config/lint/lint.xml")
        checkAllWarnings = true
        abortOnError = true
        checkReleaseBuilds = false
    }
}

kover {
    currentProject {
        createVariant("unit") { add("fossDebug") }
    }
}

// Every library the APK ships, against the licences Corvene (MIT) can carry.
//   ./gradlew :app:licenseeAndroidFossRelease
licensee {
    allow("Apache-2.0")
    allow("MIT")
    allow("BSD-2-Clause")
    allow("BSD-3-Clause")
    allow("ISC")
    allowDependency("net.java.dev.jna", "jna", libs.versions.jna.get()) {
        because("dual LGPL-2.1-or-later / Apache-2.0; Corvene takes it under Apache-2.0")
    }
}

dependencies {
    "lintChecks"(libs.compose.lint.checks)
}

// The licence texts for Settings › About (assets/notices/), and per variant
// checkApkSize<Variant>: the APK's parts against app/apk-budget.txt.
val notices = tasks.register("collectNotices", NoticesTask::class.java) {
    description = "Copies the NOTICE files and font licences into the APK's assets."
    notices.from(
        File(workspaceDir, "NOTICE"),
        rootProject.file("NOTICE"),
        rootProject.file("core/design/licenses/Inter-OFL.txt"),
        rootProject.file("core/design/licenses/JetBrainsMono-OFL.txt"),
    )
    outputDir.set(layout.buildDirectory.dir("generated/notices"))
}
extensions.getByType(ApplicationAndroidComponentsExtension::class.java).onVariants { variant ->
    variant.sources.assets?.addGeneratedSourceDirectory(notices, NoticesTask::outputDir)
    tasks.register("checkApkSize${variant.name.capitalized()}", ApkBudgetTask::class.java) {
        description = "Reports the ${variant.name} APK's size by part against apk-budget.txt."
        group = "verification"
        apkDir.set(variant.artifacts.get(SingleArtifact.APK))
        loader.set(variant.artifacts.getBuiltArtifactsLoader())
        budget.set(layout.projectDirectory.file("apk-budget.txt"))
        enforce.set(variant.buildType != "debug")
        buildType.set(variant.buildType ?: "debug")
        report.set(layout.buildDirectory.file("reports/apk-size/${variant.name}.txt"))
    }
}
