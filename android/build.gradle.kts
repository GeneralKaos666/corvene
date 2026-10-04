// Root of Corvene's Android build. The modules configure themselves through
// the convention plugins in build-logic/; nothing here reaches into them
// (cross-project configuration breaks the configuration cache).

plugins {
    id("corvene.root")
}

dependencies {
    listOf(
        ":app", ":core:common", ":core:design", ":core:ffi", ":core:platform",
        ":feature:repositories", ":feature:changes", ":feature:branches", ":feature:history", ":feature:mco", ":feature:settings",
    )
        .forEach { "kover"(project(it)) }
}

/** The Android modules, by path. Task names carry the flavour: `testFossDebugUnitTest`. */
val androidModules = listOf(
    ":core:common", ":core:design", ":core:ffi", ":core:platform",
    ":feature:repositories", ":feature:changes", ":feature:branches", ":feature:history", ":feature:mco", ":feature:settings",
    ":app",
)

// Every unit test (Robolectric, Compose, Roborazzi captures, Konsist), under one name.
tasks.register("unitTests") {
    group = "verification"
    description = "Runs every module's fossDebug unit tests and the architecture rules."
    dependsOn(androidModules.map { "$it:testFossDebugUnitTest" })
    dependsOn(":tools:architecture:test")
}

// Every analyser: type-resolved detekt per module, Android Lint, the
// architecture rules. `buildHealth` (dependency advice) is separate: it
// compiles every module again and only advises.
tasks.register("staticAnalysis") {
    group = "verification"
    description = "Runs detekt (with type resolution), Android Lint and the Konsist rules."
    dependsOn(androidModules.map { "$it:detektFossDebug" })
    dependsOn(":app:lintFossDebug")
    dependsOn(":tools:architecture:test")
}
