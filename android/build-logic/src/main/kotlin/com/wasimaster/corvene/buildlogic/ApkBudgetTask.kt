package com.wasimaster.corvene.buildlogic

import com.android.build.api.variant.BuiltArtifactsLoader
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import java.io.File
import java.util.Locale
import java.util.zip.ZipFile

/**
 * What the APK is made of, against the budget in `app/apk-budget.txt`
 * (design §5: arm64 foss ≤ 40 MB; git + ssh + lfs, the engine, JNA, dex,
 * fonts). Sizes are the bytes each part takes inside the APK (compressed
 * where the APK compresses); the total is the file's size. The report goes
 * to `build/reports/apk-size/<variant>.txt`; a part over budget fails the
 * task for single-ABI, non-debuggable variants and only warns otherwise
 * (debug builds carry an unoptimised engine).
 *
 *     ./gradlew :app:checkApkSizeFossFast -Pcorvene.abis=arm64-v8a
 */
abstract class ApkBudgetTask : DefaultTask() {

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val apkDir: DirectoryProperty

    @get:Internal
    abstract val loader: Property<BuiltArtifactsLoader>

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val budget: RegularFileProperty

    /** Over budget fails (non-debuggable variants); otherwise it warns. */
    @get:Input
    abstract val enforce: Property<Boolean>

    /** The build type: a `<buildType>.<part>` line of the budget overrides `<part>`. */
    @get:Input
    abstract val buildType: Property<String>

    @get:OutputFile
    abstract val report: RegularFileProperty

    @TaskAction
    fun check() {
        val limits = parseBudget(budget.get().asFile)
        val artifacts = loader.get().load(apkDir.get()) ?: error("no APKs in ${apkDir.get()}")
        val lines = mutableListOf<String>()
        val over = mutableListOf<String>()
        for (artifact in artifacts.elements) {
            val apk = File(artifact.outputFile)
            val parts = sortedMapOf<String, Long>()
            val abis = sortedSetOf<String>()
            ZipFile(apk).use { zip ->
                zip.entries().asSequence().filterNot { it.isDirectory }.forEach { entry ->
                    if (entry.name.startsWith("lib/")) abis += entry.name.split('/')[1]
                    parts.merge(partOf(entry.name), entry.compressedSize.coerceAtLeast(0), Long::plus)
                }
            }
            val total = apk.length()
            lines += "${apk.name} (${abis.joinToString().ifEmpty { "no native code" }})"
            lines += String.format(Locale.ROOT, "  %-10s %10s %10s", "part", "MB", "budget")
            (parts.entries.map { it.key to it.value } + ("total" to total)).forEach { (part, bytes) ->
                val limit = limits["${buildType.get()}.$part"] ?: limits[part]
                val mb = bytes / MB
                val flag = if (limit != null && mb > limit) " OVER" else ""
                lines += String.format(Locale.ROOT, "  %-10s %10.2f %10s%s", part, mb, limit?.let { "%.1f".format(Locale.ROOT, it) } ?: "-", flag)
                if (flag.isNotEmpty()) over += "$part ${"%.2f".format(Locale.ROOT, mb)} MB > ${limit} MB (${apk.name})"
            }
            if (abis.size > 1) lines += "  (the budget is for one ABI: build with -Pcorvene.abis=arm64-v8a)"
            val single = abis.size <= 1
            if (over.isNotEmpty() && enforce.get() && single) {
                report.get().asFile.writeText(lines.joinToString("\n", postfix = "\n"))
                error("APK over budget (${budget.get().asFile.name}):\n  " + over.joinToString("\n  "))
            }
        }
        val text = lines.joinToString("\n", postfix = "\n")
        report.get().asFile.writeText(text)
        logger.lifecycle(text)
        if (over.isNotEmpty()) logger.warn("APK over budget (not enforced for this variant):\n  " + over.joinToString("\n  "))
    }

    private fun parseBudget(file: File): Map<String, Double> = file.readLines()
        .map { it.substringBefore('#').trim() }
        .filter { it.isNotEmpty() }
        .associate { line ->
            val (part, mb) = line.split(Regex("\\s+"), limit = 2)
            part to mb.trim().toDouble()
        }

    companion object {
        private const val MB = 1024.0 * 1024.0

        /** The budget part an APK entry belongs to. */
        fun partOf(name: String): String {
            val file = name.substringAfterLast('/')
            return when {
                name.startsWith("lib/") && (file.startsWith("libcorvene_ffi") || file.startsWith("libcorvene-askpass")) -> "engine"
                name.startsWith("lib/") && file == "libjnidispatch.so" -> "jna"
                name.startsWith("lib/") && (file.startsWith("libgit") || file.startsWith("libssh")) -> "git"
                name.startsWith("lib/") -> "native"
                file.endsWith(".dex") -> "dex"
                file.endsWith(".ttf") || file.endsWith(".otf") -> "fonts"
                name == "resources.arsc" || name.startsWith("res/") -> "resources"
                name.startsWith("assets/") -> "assets"
                name.startsWith("META-INF/") -> "meta"
                else -> "other"
            }
        }
    }
}

/** Copies the licence texts (the repository's NOTICE, the app's, the font licences) into the APK's `assets/notices/`. */
abstract class NoticesTask : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NAME_ONLY)
    abstract val notices: org.gradle.api.file.ConfigurableFileCollection

    @get:org.gradle.api.tasks.OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun copy() {
        val dir = outputDir.get().asFile.resolve("notices")
        dir.deleteRecursively()
        dir.mkdirs()
        // in the order given: the repository's NOTICE first
        notices.files.forEachIndexed { index, file ->
            file.copyTo(dir.resolve("${index + 1}-${file.nameWithoutExtension}.txt"), overwrite = true)
        }
    }
}
