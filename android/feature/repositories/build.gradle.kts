plugins {
    id("corvene.android.feature")
}

// The repository list (GHD's Repository foldout as a screen): groups Recent,
// one per GitHub owner, Other; indicators; add, remove, select, refresh.
android {
    namespace = "com.wasimaster.corvene.repositories"
    resourcePrefix = "repo_"
}
