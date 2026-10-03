package com.wasimaster.corvene.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
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
 * The engine as `lib/<abi>/libcorvene_ffi.so` (cargo-ndk), with git's askpass
 * helper beside it as `libcorvene-askpass.so` (an executable packaged like the
 * bundled git, so it runs from the extracted library directory).
 *
 *     cargo ndk -t <abi>… -P 26 -o <out> build -p corvene-ffi --lib [--release|--profile p] [--features …]
 *
 * Debug libraries carry a gigabyte of DWARF; the copy packaged keeps the
 * symbol table only (`llvm-strip --strip-debug`), for backtraces.
 * With [prebuiltDir] set (`-Pcorvene.prebuiltRustDir`) nothing is compiled:
 * the libraries under `<dir>/<abi>` are copied instead (CI artifacts, a Rust-less checkout).
 */
abstract class CargoNdkTask @Inject constructor(
    private val exec: ExecOperations,
    private val fs: FileSystemOperations,
) : DefaultTask() {

    /** The Cargo workspace (the Corvene checkout). */
    @get:Internal
    abstract val workspaceDir: DirectoryProperty

    /** Everything cargo reads: the crates, the vendored sources, the lock file, the toolchain pin. */
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: ConfigurableFileCollection

    @get:Input
    abstract val abis: ListProperty<String>

    /** A cargo profile: `dev`, `release`, `profiling`. */
    @get:Input
    abstract val profile: Property<String>

    @get:Input
    abstract val features: ListProperty<String>

    @get:Input
    abstract val minSdk: Property<Int>

    @get:Input
    abstract val cratePackage: Property<String>

    /** The askpass helper's crate; absent = not built. */
    @get:Input
    @get:Optional
    abstract val askpassPackage: Property<String>

    @get:Internal
    abstract val ndkDir: DirectoryProperty

    /** `CARGO_TARGET_DIR`; absent = cargo's default (`<workspace>/target`). */
    @get:Internal
    abstract val cargoTargetDir: DirectoryProperty

    @get:Internal
    abstract val prebuiltDir: DirectoryProperty

    /**
     * `CARGO_PROFILE_DEV_DEBUG` for dev builds (`corvene.rustDebugInfo`,
     * default `line-tables-only`): backtraces keep file:line, and the build
     * needs a fraction of full DWARF's gigabytes on disk.
     */
    @get:Input
    abstract val devDebugInfo: Property<String>

    /** Whether [prebuiltDir] is set, as an input (its path alone must not make the task out of date). */
    @get:Input
    abstract val usesPrebuilt: Property<Boolean>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun build() {
        val out = outputDir.get().asFile
        fs.delete { delete(out) }
        out.mkdirs()
        if (prebuiltDir.isPresent) {
            copyPrebuilt(prebuiltDir.get().asFile, out)
            return
        }
        val ndk = ndkDir.get().asFile
        val workspace = workspaceDir.get().asFile
        val profileArgs = when (val p = profile.get()) {
            "dev", "debug" -> emptyList()
            "release" -> listOf("--release")
            else -> listOf("--profile", p)
        }
        val targets = abis.get().flatMap { listOf("-t", it) }
        val featureArgs = features.get().takeIf { it.isNotEmpty() }?.let { listOf("--features", it.joinToString(",")) }.orEmpty()
        cargo(
            workspace,
            ndk,
            listOf("ndk") + targets + listOf("-P", minSdk.get().toString(), "-o", out.path, "build", "-p", cratePackage.get(), "--lib") +
                profileArgs + featureArgs,
        )
        askpassPackage.orNull?.let { askpass ->
            cargo(workspace, ndk, listOf("ndk") + targets + listOf("-P", minSdk.get().toString(), "build", "-p", askpass) + profileArgs)
            val target = cargoTargetDir.orNull?.asFile ?: File(workspace, "target")
            for (abi in abis.get()) {
                val built = File(target, "${Abis.rustTriple(abi)}/${profileDir()}/$askpass")
                check(built.isFile) { "cargo built no $built" }
                built.copyTo(File(out, "$abi/lib$askpass.so"), overwrite = true).setExecutable(true)
            }
        }
        // cargo-ndk also copies the cdylib of a dependency the engine links
        // statically (android-native-keyring-store): hashed names, unused
        out.walkTopDown().filter { HASHED_CDYLIB.matches(it.name) }.forEach { it.delete() }
        val strip = File(ndk, "toolchains/llvm/prebuilt").walkTopDown().maxDepth(3).firstOrNull { it.name == "llvm-strip" }
            ?: error("no llvm-strip in $ndk")
        out.walkTopDown().filter { it.isFile && it.name.endsWith(".so") }.forEach { lib ->
            val args = if (lib.name.startsWith("libcorvene_ffi")) listOf("--strip-debug") else emptyList()
            exec.exec { commandLine(listOf(strip.path) + args + lib.path) }
        }
    }

    private companion object {
        val HASHED_CDYLIB = Regex("""lib.+-[0-9a-f]{16}\.so""")
    }

    private fun profileDir(): String = when (val p = profile.get()) {
        "dev", "debug" -> "debug"
        else -> p
    }

    private fun cargo(workspace: File, ndk: File, args: List<String>) {
        val home = System.getProperty("user.home")
        val cargoHome = System.getenv("CARGO_HOME") ?: "$home/.cargo"
        val cargo = File(cargoHome, "bin/cargo").takeIf { it.canExecute() }?.path ?: "cargo"
        exec.exec {
            workingDir(workspace)
            commandLine(listOf(cargo) + args)
            environment("ANDROID_NDK_HOME", ndk.path)
            environment("ANDROID_NDK_ROOT", ndk.path)
            environment("CARGO_INCREMENTAL", "0")
            environment("PATH", "$cargoHome/bin${File.pathSeparator}${System.getenv("PATH").orEmpty()}")
            cargoTargetDir.orNull?.let { environment("CARGO_TARGET_DIR", it.asFile.path) }
            environment("CARGO_PROFILE_DEV_DEBUG", devDebugInfo.get())
        }
    }

    private fun copyPrebuilt(from: File, out: File) {
        for (abi in abis.get()) {
            val dir = File(from, abi)
            check(File(dir, "lib${cratePackage.get().replace('-', '_')}.so").isFile) {
                "corvene.prebuiltRustDir: no ${dir.path}/lib${cratePackage.get().replace('-', '_')}.so"
            }
            fs.copy {
                from(dir)
                include("*.so")
                into(File(out, abi))
            }
        }
    }
}
