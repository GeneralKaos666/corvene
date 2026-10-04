import com.android.build.api.variant.AndroidComponentsExtension
import com.wasimaster.corvene.buildlogic.capitalized
import com.wasimaster.corvene.buildlogic.libs
import io.gitlab.arturbosch.detekt.Detekt

// Type-resolved detekt per module and debug variant (`detektFossDebug`,
// `detektPlayDebug`). detekt 1.23 targets the AGP 7/8 variant API and
// registers no `detekt<Variant>` tasks under AGP 9, so they are registered
// here by hand (WMKeyboard's approach). Each module analyses its own sources
// against its own compile classpath.

plugins {
    id("io.gitlab.arturbosch.detekt")
}

dependencies {
    "detektPlugins"(libs.detekt.formatting)
}

detekt {
    toolVersion = libs.versions.detekt.get()
    config.setFrom(rootProject.file("config/detekt/detekt.yml"))
    buildUponDefaultConfig = true
    parallel = true
    basePath = rootProject.projectDir.absolutePath
}

val components = extensions.getByType(AndroidComponentsExtension::class.java)
components.onVariants(components.selector().withBuildType("debug")) { variant ->
    val name = variant.name.capitalized()
    val variantClasspath = variant.compileClasspath
    val dirs = listOfNotNull("main", variant.flavorName?.takeIf(String::isNotEmpty))
        .flatMap { listOf("src/$it/kotlin", "src/$it/java") }
    tasks.register("detekt$name", Detekt::class.java) {
        description = "Runs detekt with type resolution over this module's $name sources."
        group = "verification"
        setSource(files(dirs.map { layout.projectDirectory.dir(it) }))
        include("**/*.kt")
        exclude("**/build/**")
        classpath.setFrom(variantClasspath, components.sdkComponents.bootClasspath)
        config.setFrom(rootProject.file("config/detekt/detekt.yml"))
        buildUponDefaultConfig = true
        parallel = true
        ignoreFailures = false
        basePath = rootProject.projectDir.absolutePath
        jvmTarget = "17"
        reports {
            html.outputLocation.set(layout.buildDirectory.file("reports/detekt/detekt$name.html"))
            sarif.outputLocation.set(layout.buildDirectory.file("reports/detekt/detekt$name.sarif"))
            xml.required.set(false)
            md.required.set(false)
            txt.required.set(false)
        }
    }
}
