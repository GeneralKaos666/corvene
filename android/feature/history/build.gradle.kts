plugins {
    id("corvene.android.feature")
}

// The History tab (GHD's CompareSidebar + SelectedCommit): the paged commit
// list with tags, multi-select and the commit menu, compare to a branch
// (Ahead / Behind, merge), and the selected commit's header and files.
android {
    namespace = "com.wasimaster.corvene.history"
    resourcePrefix = "hist_"
}
