// The tree-sitter grammars for the play flavour: an on-demand feature module
// that Google Play installs when the user asks for them in Options ›
// Advanced (Play forbids code downloaded from anywhere else, so the packs
// the foss flavour downloads are not an option there).
//
// It has no code of its own. `packaging/android/grammars.sh` fills
// src/main/jniLibs with one library per grammar unit
// (libcorvane_ts_<unit>.so, built by tools/ts-queries/build_unit.py) and
// their index (libcorvane_ts_index.so, a JSON file under a library's name
// so the installer extracts it next to them).

plugins {
    id("com.android.dynamic-feature")
}

android {
    namespace = "com.wasimaster.corvane.grammars"
    compileSdk = 35

    defaultConfig {
        minSdk = 26
    }

    flavorDimensions += "distribution"
    productFlavors {
        create("foss") {
            dimension = "distribution"
        }
        create("play") {
            dimension = "distribution"
        }
    }

    packaging {
        jniLibs {
            // opened by path: they have to be files on disk
            useLegacyPackaging = true
            keepDebugSymbols += "**/*.so"
        }
    }
}

dependencies {
    implementation(project(":app"))
}
