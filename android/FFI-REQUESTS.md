# What the Kotlin app needs from `crates/corvene-ffi`

Requests from the Android side; the Rust side owns the crate. Each says what
Kotlin does meanwhile.

1. **Repository list groups in the view model.** `RepoListVm` gives the flat
   list and `recent`; Kotlin re-implements GHD's `groupRepositories`
   (Recent, one group per GitHub owner, Other) in
   `feature/repositories/.../RepositoryGroups.kt`, without flag
   `209-recent-repositories-count` (fixed at 3) and without the filters
   (207/208/211). Wanted: `RepoListVm.groups: Vec<RepoGroupVm { kind, title, ids }>`
   computed by the same code as `corvene-ui/src/repository_list.rs::groups`,
   and a `filter: String` argument to `repo_list`.
2. **Row icons.** GHD draws lock / fork / device-desktop per repository;
   `RepoVm` has no `private`/`fork` (Kotlin shows repo vs device-desktop only).
   Wanted: `fork: bool`, `private: bool`, and `alias: Option<String>` (rename UI).
3. ~~Design style and colour mode in the settings~~: done (`settings()`,
   `setDesignStyle`, `setTheme`); the DataStore workaround is gone.
4. **App visibility.** `app_visible(bool)` so the engine pauses the indicator
   updater, fetcher and watcher while the app is in the background
   (`ProcessLifecycleOwner` ON_START/ON_STOP, design §4). Kotlin calls
   `focus()` on resume only.
5. **Start-up readiness.** The splash screen waits for the first `repo_list()`.
   A cheap `ready() -> bool` (store loaded, first indicators requested) would
   let the splash end without building a view model.
6. **`kotlin_target_version = "2.2.20"`** in `uniffi.toml` (named in
   design-core-host.md) so the generated code never uses newer language features
   than AGP 9.3's built-in Kotlin.
7. ~~Android-only compile error~~ (`set_bridge` wanted `Box<dyn Bridge>`,
   api.rs:88): fixed upstream. Lesson: `cargo check` on the host skips the
   `cfg(target_os = "android")` paths; run `cargo ndk -t arm64-v8a check -p corvene-ffi`
   (or `./gradlew :core:ffi:cargoNdkDebug`) after touching them.
8. ~~`CoreError::Failed { message }` broke the Kotlin bindings~~ (a field named
   `message` clashes with `Throwable.message`): renamed to `reason` upstream.
9. **Indicators at start-up.** After a cold start `repo_list()` has no branch or
   change indicators until `refresh_indicators()` is called (GHD's
   `RepositoryIndicatorUpdater` runs right away and then periodically).
   MainActivity calls `refreshIndicators()` once per process start meanwhile.

## Provided by the FFI (2026-10-04, 79678bd4)

- `com.wasimaster.corvene.ffi.NativeContext` must exist in `:core:ffi` as
  `object NativeContext { external fun attach(context: android.content.Context) }`
  (JNI name `Java_com_wasimaster_corvene_ffi_NativeContext_attach`) and be
  called with the application context before `Corvene(...)` or
  `headlessFetch(...)`: it hands the JVM and context to `ndk_context` for the
  Keystore-backed token store. Without it sign-in tokens cannot be saved.
- `headlessFetch(filesDir): HeadlessOutcome` (Fetched | Skipped | Failed) and
  `endHeadless()` for `CorveneFetchWorker` when no `Corvene` is alive.
- `toggleFilterOption`, `clearFilterOptions`, `stashAllChanges`, `popStash`,
  `dropStash`, `ignoreFiles`, `startAmending`, `stopAmending`,
  `setHideWhitespaceInDiff`, `RepoListVm.groups`, `RepoVm.owner/fork/private/alias`,
  `appVisible`, `ready`, start-up background tasks (886e67b8, 5cabc4ef).
