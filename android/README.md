# Corvene for Android (Kotlin + Compose)

The native Android app: a Jetpack Compose UI over Corvene's Rust engine
(`crates/corvene-ffi`, called through UniFFI's Kotlin bindings on JNA). The
GPUI build in `packaging/android/` keeps working until this app reaches parity
(milestone M-A6). Design: `.docs/android/design-compose-app.md`; status and
measurements: `NOTES.md`; what the Kotlin side needs from the engine:
`FFI-REQUESTS.md`.

## Build, install, run

Needs JDK 17+ (Android Studio's JBR is fine), the Android SDK with platform
36.1, NDK 27.3.13750724, Rust (the workspace's `rust-toolchain.toml`) with the
Android targets, `cargo install cargo-ndk` (4.1.2), and the bundled git built
once by `packaging/android/git/build.sh` (or copied from a checkout that has it).

```sh
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
export ANDROID_HOME=$HOME/Library/Android/sdk
cd android
./gradlew :app:assembleFossDebug                 # app/build/outputs/apk/foss/debug/
./gradlew :app:installFossDebug                  # onto the connected device
adb shell am start -W -n com.wasimaster.corvene.compose/com.wasimaster.corvene.MainActivity
adb logcat -s corvene                            # the engine and the app log under one tag
```

The first build compiles the engine (`cargoNdkDebug`, several minutes); later
builds reuse cargo's output. Debug builds install as
`com.wasimaster.corvene.compose`, next to the GPUI app and the release build.

| Task | What |
|---|---|
| `:app:assemble{Foss,Play}{Debug,Release,Fast}` | APKs. `fast` = release without R8, one ABI, not debuggable |
| `unitTests` | every module's fossDebug unit tests (JVM, Robolectric, Compose, Roborazzi capture) + Konsist rules |
| `staticAnalysis` | type-resolved detekt per module, Android Lint (`:app`, checkAll), Konsist rules |
| `recordRoborazziFossDebug` | every module's screenshots to its `src/test/screenshots/` |
| `verifyRoborazziFossDebug` | compare against them |
| `:app:checkElfAlignmentFossDebug` | every 64-bit `.so` in the APK aligned to 16 KB |
| `:app:licenseeAndroidFossRelease` | licence audit of what the APK ships |
| `buildHealth` | unused / mis-scoped dependencies (advice only) |
| `koverHtmlReportUnit` | merged unit-test coverage |
| `:core:ffi:cargoNdkDebug`, `:core:ffi:generateUniffiDebug`, `:core:ffi:syncBundledGitDebug` | the native pipeline's steps on their own |

## Properties

`-P<name>=…`, a line in `android/local.properties`, or the environment
(`corvene.rustProfile` → `CORVENE_RUST_PROFILE`).

| Property | Default | |
|---|---|---|
| `corvene.abis` | `arm64-v8a` for debug and fast, all four for release | ABIs of the engine, the bundled git and the APK |
| `corvene.rustProfile` | `dev` (debug), `release` (release, fast) | cargo profile; `profiling` = optimised with symbols |
| `corvene.rustFeatures` | none | cargo features of corvene-ffi (`full` = every grammar compiled in) |
| `corvene.prebuiltRustDir` | unset | copy `<dir>/<abi>/*.so` instead of running cargo |
| `corvene.cargoTargetDir` | the workspace's `target/` | `CARGO_TARGET_DIR` for the cargo tasks |
| `corvene.rustDebugInfo` | `line-tables-only` | `CARGO_PROFILE_DEV_DEBUG` of dev builds (`full` for a debugger) |
| `corvene.uniffiBindgen` | `workspace` | `standalone` generates with `tools/uniffi-bindgen` instead of `cargo run -p corvene-ffi --bin uniffi-bindgen`: no host build of the engine |
| `corvene.rustWorkspace` | the checkout above `android/` | the Cargo workspace the engine is built from |
| `corvene.bundledGitDir` | `../packaging/android/app/src/main/jniLibs` | where `lib{git,git-*,ssh,ssh-keygen}.so` live per ABI |
| `corvene.idSuffix` | `.compose` | `applicationIdSuffix` of debug builds |
| `corvene.versionCode` | `1` (gradle.properties) | versionName comes from `../Cargo.toml` |
| `corvene.splitApks` | `false` | one APK per ABI plus a universal one |
| `warningsAsErrors` | `false` | Kotlin warnings fail the build (CI) |
| `composeMetrics` | `false` | Compose compiler reports under `build/compose/` (with `--rerun`) |

A git worktree has no bundled git of its own (`jniLibs/` is ignored); point
`corvene.bundledGitDir` at the main checkout's in `local.properties`.

Debug builds also accept `--es corvene.debug.addRepository <path>` on the
`am start` line: the engine adds that folder as if it had been picked
(scripted tests on a phone without touching the system picker).

## Where things are

```
settings.gradle.kts  build.gradle.kts (unitTests, staticAnalysis)  gradle/libs.versions.toml (every version, with reasons)
build-logic/                      convention plugins, reading the catalog
  corvene.android.{library,application,compose,feature}   corvene.{detekt,rust,screenshots,root,kotlin-options}
  com/wasimaster/corvene/buildlogic/{CargoNdkTask,GenerateUniffiTask,BundledGitSyncTask,ElfAlignmentTask}.kt
config/  detekt/detekt.yml  lint/lint.xml  compose/stability.conf
core/common      logging (tag `corvene`), trace sections (`Corvene:*`), LiveState
core/design      DesignStyle, PrimerColors + palettes (3 styles × light/dark × contrast), CorveneTheme, DiffPalette,
                 Inter + JetBrains Mono, Octicons, the Primer component set (buttons, ActionList/ActionMenu, SelectPanel,
                 PrimerDialog, Flash, labels, Avatar (Coil), inputs, SegmentedControl, UnderlineNav, chips, Truncate/FilePath,
                 PrimerTopAppBar, RepositoryTopChrome, StyleMiniature)
core/ffi         the engine: generated bindings (build/generated/uniffi), Core, rememberCoreQuery, HostEventsBridge
core/platform    HostRequestHandler (URLs, clipboard, toasts, folder picker), FolderResolver
feature/repositories   RepositoryListScreen, RepositoryPicker (SelectPanel) (+ route, grouping, tests, screenshots)
feature/changes  ChangesScreen (+ CommitPanel/CommitForm, filters), DiffScreen (+ DiffPager, DiffLineCache), routes
feature/branches BranchSheet (groups, pull requests, menu), branch dialogs, sync button model/menu/controller, remote dialogs
feature/history  HistoryScreen (+ CommitPager, compare, commit menu, multi-select), CommitDetailScreen, history dialogs
feature/mco      ConflictsScreen, MultiCommitOperationRoute (choose branch, progress, abort), ChooseBranchSheet
feature/settings AppearanceScreen (style cards, theme)
app              CorveneApp (process lifecycle → appVisible/focus), MainActivity (splash, edge to edge), Navigation 3,
                 CorveneScaffold (repository chrome, list-detail on medium/expanded), PopupHost (PopupDialog), BannerFlash
tools/architecture     Konsist rules        tools/octicons/gen.py + icons.txt        tools/tokens/gen.py
```

Generated files that are committed: `core/design/.../Palettes.kt`
(`python3 tools/tokens/gen.py`), `core/design/.../Octicons.kt` and
`res/drawable/cvd_oct_*.xml` (`python3 tools/octicons/gen.py`). The UniFFI
bindings are never committed; Gradle generates them from the built library.

## Conventions

Kotlin style per `.editorconfig` (WMKeyboard's, ktlint android_studio, 140
columns), no auto-formatter. No ViewModel and no DI: the engine is the state,
`Core` is wired explicitly and handed down with `LocalCore`; screens take view
models and lambdas, routes query. Strings live in each module's `strings.xml`
with its resource prefix (`cvd_`, `repo_`, `chg_`, `br_`, `hist_`, `mco_`, `set_`, `plt_`, `app_`). Every task name
carries the flavour (`testFossDebugUnitTest`); use the root aggregates.
