package com.wasimaster.corvene.buildlogic

import org.gradle.accessors.dm.LibrariesForLibs
import org.gradle.api.Project
import org.gradle.kotlin.dsl.the
import java.io.File
import java.util.Properties

/** The main build's version catalog (gradle/libs.versions.toml), type-safe. */
val Project.libs: LibrariesForLibs get() = the()

/** The Corvene checkout: the directory above `android/`. */
val Project.workspaceDir: File get() = rootDir.parentFile

private fun Project.localProperties(): Properties = Properties().apply {
    val file = rootProject.file("local.properties")
    if (file.isFile) file.inputStream().use(::load)
}

/**
 * A build property: `-P<name>`, then `local.properties`, then the environment
 * (`corvene.rustProfile` → `CORVENE_RUST_PROFILE`), else null.
 */
fun Project.buildProperty(name: String): String? {
    val env = name.replace('.', '_').replace(Regex("([a-z])([A-Z])"), "$1_$2").uppercase()
    return providers.gradleProperty(name).orNull
        ?: localProperties().getProperty(name)
        ?: System.getenv(env)
}

/** [buildProperty] as a switch, off unless set to `true`. */
fun Project.flag(name: String): Boolean = buildProperty(name)?.toBoolean() ?: false

/** The ABIs Corvene builds for. */
object Abis {
    val ALL = listOf("arm64-v8a", "armeabi-v7a", "x86_64", "x86")

    /** `-Pcorvene.abis=a,b`, else the phone in your hand for debug builds and every ABI otherwise. */
    fun of(project: Project, buildType: String?): List<String> =
        project.buildProperty("corvene.abis")?.split(',')?.map(String::trim)?.filter(String::isNotEmpty)
            ?: if (buildType == "debug") listOf("arm64-v8a") else ALL

    fun rustTriple(abi: String): String = when (abi) {
        "arm64-v8a" -> "aarch64-linux-android"
        "armeabi-v7a" -> "armv7-linux-androideabi"
        "x86_64" -> "x86_64-linux-android"
        "x86" -> "i686-linux-android"
        else -> error("unknown ABI $abi")
    }
}

/** Shared numbers of every Android module. */
object AndroidConfig {
    const val COMPILE_SDK = 36
    const val COMPILE_SDK_MINOR = 1
    const val MIN_SDK = 26
    const val TARGET_SDK = 36
    const val NDK_VERSION = "27.3.13750724"
    const val DIMENSION = "distribution"
    val FLAVOURS = listOf("foss", "play")
}

fun String.capitalized(): String = replaceFirstChar { it.uppercase() }
