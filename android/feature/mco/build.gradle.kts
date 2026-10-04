plugins {
    id("corvene.android.feature")
}

// Multi-commit operations (GHD's MultiCommitOperation popup): choose a
// branch, the force-push warning, progress, the conflicts screen with
// ours / theirs per file, abort; merge's branch chooser.
android {
    namespace = "com.wasimaster.corvene.mco"
    resourcePrefix = "mco_"
}
