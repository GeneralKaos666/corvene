import com.android.build.api.artifact.SingleArtifact
import com.android.build.api.dsl.LibraryExtension
import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import com.android.build.api.variant.LibraryAndroidComponentsExtension
import com.wasimaster.corvene.buildlogic.Abis
import com.wasimaster.corvene.buildlogic.AndroidConfig
import com.wasimaster.corvene.buildlogic.BundledGitSyncTask
import com.wasimaster.corvene.buildlogic.CargoNdkTask
import com.wasimaster.corvene.buildlogic.ElfAlignmentTask
import com.wasimaster.corvene.buildlogic.GenerateUniffiTask
import com.wasimaster.corvene.buildlogic.buildProperty
import com.wasimaster.corvene.buildlogic.capitalized
import com.wasimaster.corvene.buildlogic.libs
import com.wasimaster.corvene.buildlogic.workspaceDir

// The native side, wired through the Variant API.
//
// In the library that owns the bindings (:core:ffi), per build type:
//   cargoNdk<BuildType>        crates/corvene-ffi → jniLibs/<abi>/libcorvene_ffi.so (+ libcorvene-askpass.so)
//   generateUniffi<BuildType>  the Kotlin bindings from that library → generated Kotlin sources
//   syncBundledGit<BuildType>  lib{git,git-*,ssh,ssh-keygen}.so from packaging/android → jniLibs
// The flavours share them: foss and play run the same engine.
//
// In the application, per variant:
//   checkElfAlignment<Variant> every 64-bit .so in the APK aligned to 16 KB
//
// Properties (-P, local.properties or the environment, see README.md):
//   corvene.abis             ABIs (default arm64-v8a for debug, all four otherwise)
//   corvene.rustProfile      cargo profile (default dev for debug, release otherwise)
//   corvene.rustFeatures     cargo features of corvene-ffi, comma separated
//   corvene.prebuiltRustDir  copy <dir>/<abi>/*.so instead of running cargo
//   corvene.cargoTargetDir   CARGO_TARGET_DIR (default: the workspace's target/)
//   corvene.rustDebugInfo    CARGO_PROFILE_DEV_DEBUG of dev builds (default line-tables-only)
//   corvene.uniffiBindgen    `standalone`: generate with tools/uniffi-bindgen (no host build of the engine)
//   corvene.rustWorkspace    the Cargo workspace to build (default: the checkout above android/)
//   corvene.bundledGitDir    the bundled git per ABI (default packaging/android/app/src/main/jniLibs)

val workspace = layout.projectDirectory.dir(buildProperty("corvene.rustWorkspace") ?: workspaceDir.path)
val cargoTarget = buildProperty("corvene.cargoTargetDir")?.let { layout.projectDirectory.dir(it) }
val rustDebugInfo = buildProperty("corvene.rustDebugInfo") ?: "line-tables-only"
val prebuilt = buildProperty("corvene.prebuiltRustDir")?.let { layout.projectDirectory.dir(it) }

pluginManager.withPlugin("com.android.library") {
    extensions.configure<LibraryExtension> { ndkVersion = AndroidConfig.NDK_VERSION }
    val components = extensions.getByType(LibraryAndroidComponentsExtension::class.java)

    fun cargoTask(buildType: String) = tasks.register("cargoNdk${buildType.capitalized()}", CargoNdkTask::class.java) {
        description = "Builds libcorvene_ffi.so and the askpass helper for $buildType."
        group = "corvene"
        workspaceDir.set(workspace)
        sources.from(
            workspace.file("Cargo.toml"),
            workspace.file("Cargo.lock"),
            workspace.file("rust-toolchain.toml"),
            workspace.dir(".cargo"),
            fileTree(workspace.dir("crates")) { exclude("**/target/**") },
            fileTree(workspace.dir("vendor")) { exclude("**/target/**", "**/.git/**") },
        )
        abis.set(Abis.of(project, buildType))
        profile.set(buildProperty("corvene.rustProfile") ?: if (buildType == "debug") "dev" else "release")
        features.set(buildProperty("corvene.rustFeatures")?.split(',')?.map(String::trim)?.filter(String::isNotEmpty).orEmpty())
        minSdk.set(AndroidConfig.MIN_SDK)
        cratePackage.set("corvene-ffi")
        askpassPackage.set("corvene-askpass")
        ndkDir.set(components.sdkComponents.ndkDirectory)
        cargoTarget?.let(cargoTargetDir::set)
        prebuilt?.let(prebuiltDir::set)
        usesPrebuilt.set(prebuilt != null)
        devDebugInfo.set(rustDebugInfo)
        outputDir.set(layout.buildDirectory.dir("generated/rust/$buildType/jniLibs"))
    }

    fun uniffiTask(buildType: String, cargo: TaskProvider<CargoNdkTask>) =
        tasks.register("generateUniffi${buildType.capitalized()}", GenerateUniffiTask::class.java) {
            description = "Generates the UniFFI Kotlin bindings from the $buildType library."
            group = "corvene"
            workspaceDir.set(workspace)
            nativeLibs.set(cargo.flatMap { it.outputDir })
            config.set(workspace.file("crates/corvene-ffi/uniffi.toml"))
            crateManifest.set(workspace.file("crates/corvene-ffi/Cargo.toml"))
            uniffiVersion.set(libs.versions.uniffi)
            libraryName.set("corvene_ffi")
            devDebugInfo.set(rustDebugInfo)
            cargoTarget?.let(cargoTargetDir::set)
            outputDir.set(layout.buildDirectory.dir("generated/uniffi/$buildType/kotlin"))
            if (buildProperty("corvene.uniffiBindgen") == "standalone") {
                standaloneManifest.set(rootProject.layout.projectDirectory.file("tools/uniffi-bindgen/Cargo.toml"))
            }
        }

    fun gitTask(buildType: String) = tasks.register("syncBundledGit${buildType.capitalized()}", BundledGitSyncTask::class.java) {
        description = "Copies the bundled git for $buildType's ABIs."
        group = "corvene"
        val dir = buildProperty("corvene.bundledGitDir")?.let { layout.projectDirectory.dir(it) }
            ?: workspace.dir("packaging/android/app/src/main/jniLibs")
        sourceDir.set(dir)
        buildScript.set(workspace.dir("packaging/android/git"))
        val selected = Abis.of(project, buildType)
        abis.set(selected)
        libraries.from(fileTree(dir) { selected.forEach { abi -> BundledGitSyncTask.PATTERNS.forEach { include("$abi/$it") } } })
        outputDir.set(layout.buildDirectory.dir("generated/git/$buildType/jniLibs"))
    }

    val byBuildType = mutableMapOf<String, Triple<TaskProvider<CargoNdkTask>, TaskProvider<GenerateUniffiTask>, TaskProvider<BundledGitSyncTask>>>()
    components.onVariants { variant ->
        val buildType = variant.buildType ?: "debug"
        val (cargo, uniffi, git) = byBuildType.getOrPut(buildType) {
            val cargo = cargoTask(buildType)
            Triple(cargo, uniffiTask(buildType, cargo), gitTask(buildType))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdkTask::outputDir)
        variant.sources.jniLibs?.addGeneratedSourceDirectory(git, BundledGitSyncTask::outputDir)
        variant.sources.kotlin?.addGeneratedSourceDirectory(uniffi, GenerateUniffiTask::outputDir)
    }
}

pluginManager.withPlugin("com.android.application") {
    val components = extensions.getByType(ApplicationAndroidComponentsExtension::class.java)
    components.onVariants { variant ->
        tasks.register("checkElfAlignment${variant.name.capitalized()}", ElfAlignmentTask::class.java) {
            description = "Checks that the ${variant.name} APK's 64-bit native libraries are aligned to 16 KB."
            group = "verification"
            apkDir.set(variant.artifacts.get(SingleArtifact.APK))
            loader.set(variant.artifacts.getBuiltArtifactsLoader())
            failOnMisaligned.set(true)
            report.set(layout.buildDirectory.file("reports/elf-alignment/${variant.name}.txt"))
        }
    }
}
