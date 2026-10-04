package com.wasimaster.corvene.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.provider.ListProperty
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import java.io.File
import javax.inject.Inject

/**
 * The bundled git (git, its HTTPS helper and shell parts, ssh, ssh-keygen,
 * git-lfs) as `lib/<abi>/lib*.so`, from where `packaging/android/git/build.sh`
 * leaves them (`packaging/android/app/src/main/jniLibs/<abi>`, or
 * `-Pcorvene.bundledGitDir`). They are never rebuilt here; an ABI without them
 * runs build.sh once (needs make, perl and Go).
 */
abstract class BundledGitSyncTask @Inject constructor(
    private val exec: ExecOperations,
    private val fs: FileSystemOperations,
) : DefaultTask() {

    @get:Internal
    abstract val sourceDir: DirectoryProperty

    @get:Internal
    abstract val buildScript: DirectoryProperty

    @get:Input
    abstract val abis: ListProperty<String>

    /** The libraries copied, as inputs (resolved lazily from [sourceDir]). */
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val libraries: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun sync() {
        val source = sourceDir.get().asFile
        val missing = abis.get().filterNot { File(source, "$it/libgit.so").isFile }
        if (missing.isNotEmpty()) {
            val script = File(buildScript.get().asFile, "build.sh")
            check(script.canExecute()) {
                "no bundled git for $missing in $source and no $script: set corvene.bundledGitDir"
            }
            exec.exec { commandLine(listOf(script.path) + missing) }
        }
        fs.sync {
            from(source) {
                abis.get().forEach { abi -> PATTERNS.forEach { include("$abi/$it") } }
            }
            into(outputDir)
        }
    }

    companion object {
        val PATTERNS = listOf("libgit.so", "libgit-*.so", "libssh.so", "libssh-keygen.so")
    }
}
