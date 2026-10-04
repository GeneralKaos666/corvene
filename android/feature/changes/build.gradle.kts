plugins {
    id("corvene.android.feature")
}

// The Changes tab (GHD's ChangesSidebar + the diff of the selected file):
// the file list with include toggles, filters and swipe actions, the commit
// panel, and the paged, highlighted unified diff.
android {
    namespace = "com.wasimaster.corvene.changes"
    resourcePrefix = "chg_"
}

