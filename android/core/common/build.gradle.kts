plugins {
    id("corvene.android.library")
    id("corvene.android.compose")
}

android {
    namespace = "com.wasimaster.corvene.common"
    resourcePrefix = "cmn_"
}

dependencies {
    implementation(libs.androidx.tracing)
    testImplementation(libs.junit)
}
