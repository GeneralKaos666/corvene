package com.wasimaster.corvene.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import java.io.File
import javax.inject.Inject

/**
 * The Kotlin bindings (`com.wasimaster.corvene.ffi.gen`) from the metadata in
 * a built `libcorvene_ffi.so`, by the generator built from the same `uniffi`
 * as the library:
 *
 *     cargo run -p corvene-ffi --bin uniffi-bindgen -- generate --library <so> --language kotlin --out-dir <out>
 *
 * Fails when the crate's `uniffi` is not the version the catalog names, so a
 * generator/runtime mismatch shows up here rather than as a crash on device.
 */
abstract class GenerateUniffiTask @Inject constructor(
    private val exec: ExecOperations,
    private val fs: FileSystemOperations,
) : DefaultTask() {

    @get:Internal
    abstract val workspaceDir: DirectoryProperty

    /** CargoNdkTask's output: `<abi>/libcorvene_ffi.so`. Any one ABI's library serves. */
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val nativeLibs: DirectoryProperty

    /**
     * The crate's uniffi.toml. An input only: in library mode UniFFI 0.32 finds
     * it through `cargo metadata` (run in the workspace), and `--config` now
     * wants the new global format.
     */
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val config: RegularFileProperty

    /** The crate manifest, for the version check. */
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val crateManifest: RegularFileProperty

    @get:Input
    abstract val uniffiVersion: Property<String>

    @get:Input
    abstract val libraryName: Property<String>

    @get:Internal
    abstract val cargoTargetDir: DirectoryProperty

    /**
     * Absent: `cargo run -p corvene-ffi --bin uniffi-bindgen` in the workspace.
     * Set: the manifest of a standalone generator crate (`-Pcorvene.uniffiBindgen=standalone`
     * → tools/uniffi-bindgen), which needs no host build of the engine.
     */
    @get:InputFile
    @get:Optional
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val standaloneManifest: RegularFileProperty

    /** As [CargoNdkTask.devDebugInfo]: the generator is a dev build too. */
    @get:Input
    abstract val devDebugInfo: Property<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun generate() {
        val manifest = crateManifest.get().asFile.readText()
        val declared = Regex("""(?m)^uniffi\s*=\s*(?:"([^"]+)"|\{[^}]*version\s*=\s*"([^"]+)")""").find(manifest)
            ?.groupValues?.drop(1)?.firstOrNull { it.isNotEmpty() }
        check(declared == uniffiVersion.get()) {
            "crates/corvene-ffi uses uniffi $declared, the catalog says ${uniffiVersion.get()}: update gradle/libs.versions.toml with the bindings"
        }
        val library = nativeLibs.get().asFile.walkTopDown().firstOrNull { it.name == "lib${libraryName.get()}.so" }
            ?: error("no lib${libraryName.get()}.so in ${nativeLibs.get().asFile}")
        val out = outputDir.get().asFile
        fs.delete { delete(out) }
        out.mkdirs()
        val home = System.getProperty("user.home")
        val cargoHome = System.getenv("CARGO_HOME") ?: "$home/.cargo"
        val cargo = File(cargoHome, "bin/cargo").takeIf { it.canExecute() }?.path ?: "cargo"
        exec.exec {
            workingDir(workspaceDir.get().asFile)
            val runner = standaloneManifest.orNull?.asFile
                ?.let { listOf("--manifest-path", it.path, "--bin", "uniffi-bindgen") }
                ?: listOf("-p", "corvene-ffi", "--bin", "uniffi-bindgen")
            commandLine(
                listOf(cargo, "run", "--quiet") + runner + listOf(
                    "--", "generate", "--library", library.path, "--language", "kotlin",
                    "--out-dir", out.path, "--no-format",
                ),
            )
            environment("CARGO_INCREMENTAL", "0")
            environment("PATH", "$cargoHome/bin${File.pathSeparator}${System.getenv("PATH").orEmpty()}")
            cargoTargetDir.orNull?.let { environment("CARGO_TARGET_DIR", it.asFile.path) }
            environment("CARGO_PROFILE_DEV_DEBUG", devDebugInfo.get())
        }
    }
}
