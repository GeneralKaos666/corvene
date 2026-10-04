// The root project: the aggregating halves of dependency analysis
// (`./gradlew buildHealth`, advice only) and coverage
// (`./gradlew koverHtmlReportUnit`, every module's fossDebug unit tests).

plugins {
    id("com.autonomousapps.dependency-analysis")
    id("org.jetbrains.kotlinx.kover")
}

extensions.configure<com.autonomousapps.DependencyAnalysisExtension> {
    issues {
        all {
            onAny { severity("warn") }
        }
    }
}
