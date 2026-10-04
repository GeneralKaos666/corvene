plugins {
    id("corvene.android.feature")
}

// Branches (GHD's branch foldout, its dialogs and the push/pull button): the
// branch SelectPanel with the pull requests tab, create / rename / delete,
// checkout with the stash prompt, the sync button's states, menu and dialogs.
android {
    namespace = "com.wasimaster.corvene.branches"
    resourcePrefix = "br_"
}
