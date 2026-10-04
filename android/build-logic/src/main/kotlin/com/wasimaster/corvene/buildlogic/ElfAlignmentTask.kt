package com.wasimaster.corvene.buildlogic

import com.android.build.api.variant.BuiltArtifactsLoader
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.ZipFile

/**
 * Every 64-bit `.so` in the variant's APKs has LOAD segments aligned to 16 KB
 * (Android 15+ devices with 16 KB pages refuse anything less). The same check
 * as the NDK's `check_elf_alignment.sh`, read from the ELF program headers
 * directly so it needs no objdump. 32-bit ABIs are reported, not checked:
 * 16 KB pages exist on arm64 and x86_64 only.
 *
 *     ./gradlew :app:checkElfAlignmentFossDebug
 */
abstract class ElfAlignmentTask : DefaultTask() {

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val apkDir: DirectoryProperty

    @get:Internal
    abstract val loader: Property<BuiltArtifactsLoader>

    @get:Input
    abstract val failOnMisaligned: Property<Boolean>

    @get:OutputFile
    abstract val report: RegularFileProperty

    @TaskAction
    fun check() {
        val artifacts = loader.get().load(apkDir.get()) ?: error("no APKs in ${apkDir.get()}")
        val lines = mutableListOf<String>()
        val misaligned = mutableListOf<String>()
        for (artifact in artifacts.elements) {
            ZipFile(File(artifact.outputFile)).use { zip ->
                zip.entries().asSequence().filter { it.name.startsWith("lib/") && it.name.endsWith(".so") }.forEach { entry ->
                    val bytes = zip.getInputStream(entry).use { it.readBytes() }
                    val align = minLoadAlignment(bytes)
                    val abi = entry.name.split('/')[1]
                    val checked = abi == "arm64-v8a" || abi == "x86_64"
                    val ok = align == null || align >= PAGE_16K
                    lines += "${if (!checked) "skip" else if (ok) "ok  " else "FAIL"} ${align ?: "-"} ${entry.name}"
                    if (checked && !ok) misaligned += "${entry.name} (2^${java.lang.Long.numberOfTrailingZeros(align ?: 1L)})"
                }
            }
        }
        report.get().asFile.writeText(lines.joinToString("\n", postfix = "\n"))
        if (misaligned.isNotEmpty()) {
            val message = "not aligned to 16 KB:\n  " + misaligned.joinToString("\n  ")
            if (failOnMisaligned.get()) error(message) else logger.warn(message)
        }
    }

    /** The smallest `p_align` of the PT_LOAD segments, or null for a file that is not ELF. */
    private fun minLoadAlignment(bytes: ByteArray): Long? {
        if (bytes.size < 64 || bytes[0] != 0x7f.toByte() || bytes[1] != 'E'.code.toByte()) return null
        val is64 = bytes[4].toInt() == 2
        val buffer = ByteBuffer.wrap(bytes).order(if (bytes[5].toInt() == 1) ByteOrder.LITTLE_ENDIAN else ByteOrder.BIG_ENDIAN)
        val phoff = if (is64) buffer.getLong(0x20) else buffer.getInt(0x1c).toLong()
        val phentsize = buffer.getShort(if (is64) 0x36 else 0x2a).toInt()
        val phnum = buffer.getShort(if (is64) 0x38 else 0x2c).toInt()
        var min: Long? = null
        for (i in 0 until phnum) {
            val base = (phoff + i.toLong() * phentsize).toInt()
            if (buffer.getInt(base) != PT_LOAD) continue
            val align = if (is64) buffer.getLong(base + 0x30) else buffer.getInt(base + 0x1c).toLong()
            min = minOf(min ?: align, align)
        }
        return min
    }

    private companion object {
        const val PT_LOAD = 1
        const val PAGE_16K = 16_384L
    }
}
