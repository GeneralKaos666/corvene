package com.wasimaster.corvene.architecture

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.container.KoScope
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Rules the compiler cannot see (.docs/android/design-compose-app.md §6),
 * checked by reading the sources with Konsist. Each is here because breaking
 * it compiles and passes every other test.
 */
class ArchitectureTest {

    private val root = File(projectRoot())

    /** The module graph points down: app → feature → core:{platform → ffi, design} → common. */
    @Test
    fun `modules depend only on the layers below them`() {
        val allowed = mapOf(
            "core/common" to emptySet(),
            "core/design" to setOf("common"),
            "core/ffi" to setOf("common"),
            "core/platform" to setOf("common", "ffi"),
            "feature/repositories" to setOf("common", "design", "ffi", "platform"),
            "feature/changes" to setOf("common", "design", "ffi", "platform"),
            "feature/branches" to setOf("common", "design", "ffi", "platform"),
            "feature/history" to setOf("common", "design", "ffi", "platform"),
            "feature/mco" to setOf("common", "design", "ffi", "platform"),
            "feature/settings" to setOf("common", "design", "ffi", "platform"),
            "feature/onboarding" to setOf("common", "design", "ffi", "platform"),
        )
        val problems = mutableListOf<String>()
        for ((module, layers) in allowed) {
            val imports = sources("$module/src/main").imports.map { it.name }
            val corvene = imports.filter { it.startsWith(PACKAGE) }
                .map { it.removePrefix(PACKAGE).substringBefore('.') }
                .toSet()
            val own = module.substringAfterLast('/')
            (corvene - layers - own).forEach { problems += "$module imports com.wasimaster.corvene.$it" }
        }
        assertTrue(problems.joinToString("\n"), problems.isEmpty())
    }

    /** :core:design renders anywhere: no engine, no platform. */
    @Test
    fun `the design system imports neither the engine nor JNA`() {
        val bad = sources("core/design/src/main").imports.map { it.name }
            .filter { it.startsWith("$PACKAGE.ffi") || it.startsWith("$PACKAGE.platform") || it.startsWith("com.sun.jna") }
        assertTrue("core:design imports $bad", bad.isEmpty())
    }

    /** Only :core:ffi touches JNA or the bindings' internals. */
    @Test
    fun `only core ffi touches JNA`() {
        val bad = production().files
            .filterNot { "/core/ffi/" in it.path }
            .filter { file -> file.imports.any { it.name.startsWith("com.sun.jna") || it.name.contains("UniffiLib") } }
            .map { it.path }
        assertTrue("JNA outside :core:ffi: $bad", bad.isEmpty())
    }

    /** No ViewModel and no DI container: the engine is the state, wiring is explicit. */
    @Test
    fun `no ViewModels and no dependency injection`() {
        val bad = production().files.filter { file ->
            file.imports.any {
                it.name.startsWith("androidx.lifecycle.ViewModel") ||
                    it.name.startsWith("androidx.lifecycle.viewmodel") ||
                    it.name.startsWith("javax.inject") ||
                    it.name.startsWith("dagger.") ||
                    it.name.startsWith("org.koin")
            }
        }.map { it.path }
        assertTrue("ViewModel or DI in $bad", bad.isEmpty())
    }

    /** Screens branch on the style only through :core:design's components. */
    @Test
    fun `the design style local is read only inside core design`() {
        val bad = production().files
            .filterNot { "/core/design/" in it.path }
            .filter { "LocalDesignStyle" in it.text }
            .map { it.path }
        assertTrue("LocalDesignStyle outside :core:design: $bad", bad.isEmpty())
    }

    /** Screens take view models and lambdas; the route next to them talks to the engine. */
    @Test
    fun `screens never take the engine`() {
        val bad = sources("feature").functions()
            .filter { fn -> fn.name.endsWith("Screen") && fn.annotations.any { it.name == "Composable" } }
            .filter { fn -> fn.parameters.any { it.type.name in setOf("Core", "Corvene") } }
            .map { it.name }
        assertTrue("screens taking the engine: $bad", bad.isEmpty())
    }

    /** Every public composable in :core:design takes a modifier. */
    @Test
    fun `design components take a modifier`() {
        val bad = sources("core/design/src/main").functions()
            .filter { fn -> fn.annotations.any { it.name == "Composable" } && !fn.hasPrivateModifier && !fn.hasInternalModifier }
            .filter { fn -> fn.name.first().isUpperCase() && fn.name != "CorveneTheme" && fn.name != "DesignStyleSamples" }
            .filterNot { fn -> fn.parameters.any { it.name == "modifier" } }
            .map { it.name }
        assertTrue("composables without a modifier: $bad", bad.isEmpty())
    }

    /** Robolectric runs a pinned SDK (4.16 has no android-all for 36.1). */
    @Test
    fun `robolectric pins its sdk in every module that uses it`() {
        val modules = root.walkTopDown().onEnter { it.name !in SKIPPED_DIRS }
            .filter { it.name == "build.gradle.kts" }
            .filter { it.readText().contains("corvene.screenshots") || it.readText().contains("libs.robolectric") }
            .map { it.parentFile }
            .toList()
        val missing = modules.filterNot { File(it, "src/test/resources/robolectric.properties").isFile }
        assertTrue("no robolectric.properties in $missing", missing.isEmpty())
    }

    /** Resource names carry their module's prefix (resourcePrefix also lints it). */
    @Test
    fun `strings carry the module prefix`() {
        val problems = mutableListOf<String>()
        root.walkTopDown().onEnter { it.name !in SKIPPED_DIRS }
            .filter { it.name == "strings.xml" && "/src/main/res/" in it.invariantSeparatorsPath }
            .forEach { file ->
                val module = file.parentFile.parentFile.parentFile.parentFile.parentFile
                val prefix = Regex("""resourcePrefix = "([^"]+)"""").find(File(module, "build.gradle.kts").readText())?.groupValues?.get(1)
                    ?: return@forEach
                Regex("""<string name="([^"]+)"""").findAll(file.readText()).map { it.groupValues[1] }
                    .filterNot { it.startsWith(prefix) }
                    .forEach { problems += "${file.relativeTo(root)}: $it" }
            }
        assertTrue(problems.joinToString("\n"), problems.isEmpty())
    }

    /** Trace sections share the `Corvene:` prefix and never repeat. */
    @Test
    fun `trace sections are prefixed and unique`() {
        val file = Konsist.scopeFromFile("core/common/src/main/kotlin/com/wasimaster/corvene/common/CorveneTrace.kt").files.single()
        val values = file.objects().single { it.name == "CorveneTrace" }.properties()
            .map { it.text.substringAfter('"').substringBefore('"') }
        assertTrue("CorveneTrace declares no sections", values.isNotEmpty())
        assertEquals("duplicate section names", values.size, values.toSet().size)
        assertTrue("sections not prefixed Corvene: $values", values.all { it.startsWith("Corvene:") })
    }

    private fun production(): KoScope = sources("app/src") + sources("core") + sources("feature")

    private fun sources(dir: String): KoScope =
        Konsist.scopeFromDirectory(dir).slice { "/build/" !in it.path && "/src/test" !in it.path && "/src/androidTest" !in it.path }

    private companion object {
        const val PACKAGE = "com.wasimaster.corvene."
        val SKIPPED_DIRS = setOf("build", ".git", ".gradle", "node_modules", ".idea", "build-logic")

        /** The directory holding settings.gradle.kts, walking up from where the test runs. */
        fun projectRoot(): String {
            var dir: File? = File("").absoluteFile
            while (dir != null && !File(dir, "settings.gradle.kts").isFile) dir = dir.parentFile
            return requireNotNull(dir) { "no settings.gradle.kts above ${File("").absolutePath}" }.path
        }
    }
}
