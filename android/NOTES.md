# Notes

Status, measurements and every place this build differs from
`.docs/android/design-compose-app.md`. Newest milestone first.

# M-A1 notes

## Built

- `:core:design`: DiffPalette + `LocalDiffPalette`, `LocalOcticonTint` (bars set
  Link; `Octicon`/`PrimerIconButton` default to it), ActionList container and
  item trailing count/check/chevron, ActionMenu (+ item, divider, group header),
  SelectPanel (ModalBottomSheet < 600 dp, anchored Popup otherwise), PrimerDialog
  (+ fullScreenOnCompact), Flash, Label/StateLabel/CounterLabel, BranchName,
  Avatar (Coil 3.2 core + compose-core, file path over initials), Spinner,
  ProgressBar, SegmentedControl, UnderlineNav, PrimerChip + ChipRow, Truncate +
  FilePath (dimmed directory, bold name, middle ellipsis), PrimerTextField,
  FilterField, PrimerCheckbox (tri-state), PrimerSwitch, SwitchRow, RadioRow,
  PrimerTopAppBar, RepositoryTopChrome (Mobile/Material: app bar + branch chip +
  sync button; Desktop: GHD toolbar), StyleMiniature, `isCompactWidth()`,
  `diffPaletteOf()`. 42 more Octicons (79).
- `:feature:changes`: ChangesScreen (filters live through `toggleFilterOption`,
  include box incl. partial, status octicons, swipe right include / left discard
  with the prompt per `confirmDiscardChanges`, stash banner with Restore =
  `popStash`, conflicts banner + badge, NoChanges blank slate with disabled
  suggestion cards), CommitPanel (Undo bar, docked "Commit to <branch>", commit
  sheet = CommitForm), DiffScreen (header with include box, +/−, gear menu: wrap
  lines, hide whitespace via `setHideWhitespaceInDiff`, Unified/Split on wide
  screens), DiffPager (200-row pages, one page of prefetch either side, LRU of 12
  pages, refresh after state changes), DiffLineCache (4096 AnnotatedStrings
  keyed generation+index, cleared per palette).
- `:feature:settings`: AppearanceScreen (three style cards drawn in their own
  style, theme radios, "pinned by flag" Flash) + route.
- `:app`: CorveneScaffold (repository chrome, Changes [n] | History tabs,
  compact single pane + `Diff` destination, medium/expanded
  NavigableListDetailPaneScaffold with a draggable divider at 35/50/65 %),
  RepositoryPicker from the title, read-only branch picker, overflow menu
  (Refresh, Appearance), PopupHost (Error dialog with git output behind "Show
  details"; any other kind = a generic dialog with its fields), process
  lifecycle → `appVisible` / `focus`. The paintbrush menu is gone.

## Deviations (and why)

- **Split view deferred**: the gear menu shows Unified/Split on wide screens;
  Split says it arrives later and the diff stays unified (FFI-REQUESTS #18).
- **No Kotlin `TextField`/`Checkbox`/`Switch` names**: `PrimerTextField`,
  `PrimerCheckbox`, `PrimerSwitch`, `RadioRow`, `SwitchRow`, to not shadow
  Material's composables in the same files.
- **Filter applied in Kotlin** (`visibleFiles`, same rule as
  `corvene-core/src/filter.rs::matches_options`); no text filter, no
  703-changes-sort-order (FFI-REQUESTS #15).
- **Commit message in `rememberSaveable`**, cleared on `commitNonce`
  (FFI-REQUESTS #13). Co-authors are read-only Labels.
- **Discard prompt is Kotlin's** PrimerDialog, not GHD's popup (FFI-REQUESTS
  #14); it discards one file (swipe).
- **Diff refresh**: the pager re-reads its window (≤ 600 rows) after every state
  change and replaces only pages that differ (FFI-REQUESTS #11).
- **Selected-line gutters** use GHD's look in every style (blue gutter = in the
  commit); the GitHub app has no line staging to copy.
- **Wrap** defaults to on below 600 dp and off above, per screen (not saved).
- **Sync button** shows GHD's states (Fetch/Pull n/Push n/Publish/progress) and is
  disabled until M-A3; the branch picker lists local branches read-only (M-A2).
- **History tab** is a placeholder blank slate (M-A4); switching tabs dispatches
  `selectSection`.
- **Window class** comes from `LocalConfiguration.screenWidthDp` (600 dp), not
  `WindowSizeClass`, so :core:design needs no adaptive dependency; medium and
  expanded both get the two panes, no NavigationRail yet.
- **`rememberCoreQuery` takes nullable results** (`<T>` instead of `<T : Any>`):
  `changes()`/`diffHeader()` answer null for an unknown repository.
- **StyleMiniature in :core:design**: a card has to draw another style's primary
  button, and only :core:design may branch on the style.
- **Moved** `feature/core/ffi/.../HostEventsBridgeTest.kt` (committed under the
  wrong root in M-A0) to `core/ffi/src/test`.
- **Highlight**: `AltVariable` and `Type` spans arrive as `Other` and stay
  uncoloured (FFI-REQUESTS #10).

# M-A0 notes

## Deviations from the design (and why)

- **Catalog**: every pinned version resolved as written (checked against Google
  Maven / Maven Central 2026-10-03). Not added yet because nothing uses them in
  M-A0: coil, work, browser, metricsPerformance, playFeatureDelivery,
  uiautomator, benchmark, androidx-tracing's -ktx. `uniffi = "0.32"` is the
  crate's requirement string, asserted by `GenerateUniffiTask`.
- **build-logic reads the catalog** through `versionCatalogs { from(../gradle/libs.versions.toml) }`
  and exposes it to the precompiled plugins as the type-safe `libs` (the
  `LibrariesForLibs` jar trick), instead of hard-coded versions like WMKeyboard's.
  The Kotlin daemon-client pin sits in build-logic's dependencies (where the
  plugins' classpath is), not the root `buildscript`.
- **No `jvmToolchain(17)`** anywhere: there is no JDK 17 on the machine and the
  foojay resolver would download one per build. JDK 21 compiles with
  `jvmTarget = 17` / Java 17 source and target.
- **Kotlin options** are set on the `KotlinCompile` tasks (`corvene.kotlin-options`)
  rather than through the `kotlin {}` extension, whose type differs between
  AGP 9's built-in Kotlin and kotlin-android.
- **Native tasks per build type, not per variant**: `cargoNdkDebug`,
  `generateUniffiDebug`, `syncBundledGitDebug` (and `…Release`) are shared by
  foss and play, since both flavours run the same engine. They live in
  `:core:ffi` (which owns the bindings); `:app` gets only
  `checkElfAlignment<Variant>`. They become per-variant if a flavour ever needs
  different cargo features.
- **`checkElfAlignment`** reads the ELF program headers itself instead of
  running the NDK's `check_elf_alignment.sh` (no objdump dependency); same rule
  (every PT_LOAD of a 64-bit `.so` aligned ≥ 16 KB).
- **No `CARGO_TARGET_DIR=$rootDir/native/target`**: the cargo tasks use the
  workspace's `target/` (shared with every other cargo job) unless
  `corvene.cargoTargetDir` is set. A separate target dir would double ~10 GB of
  artifacts on a disk that ran full twice during this milestone.
- **16 KB `max-page-size`**: no `.cargo/config.toml` change (outside android/);
  the alignment check reports what the toolchain produces (see below).
- **Host requests are a `Channel`**, not a `SharedFlow`: a request made while
  no activity collects (a picker asked for during a configuration change) waits
  instead of being dropped.
- **`Core` has no `Mode` (App/Headless)** yet: the headless fetch worker is M-A3.
- **Splash** waits for the first `settings()` + `repoList()` answer or 3 s, whichever first.
- **Design style + theme** come from the engine (`settings()`; `setDesignStyle`,
  `setTheme`), changed from a temporary top-bar menu (paintbrush icon) until
  Settings › Appearance (M-A1). Theme `HighContrast` = dark + high contrast;
  the other themes follow the system's high-contrast text setting. The
  DataStore dependency is dropped from :app (kept in the catalog for
  Kotlin-only preferences later).
- **Repository grouping in Kotlin** (`groupRepositories`), until the view model
  carries it (FFI-REQUESTS.md #1). Owner groups are sorted by owner name.
- **Fonts**: Inter 4.1 variable (880 KB, roman only) and JetBrains Mono's
  **NL** cut (no ligatures, so `liga 0` is unnecessary; regular + italic 420 KB).
  1.3 MB in all, over the design's 1.1 MB font budget; an Inter subset is an
  M-A5 size item.
- **Octicons**: 37 icons (16 + 24 px each) fetched from
  raw.githubusercontent.com/primer/octicons/main by `tools/octicons/gen.py`
  (it prefers a local `npm install @primer/octicons` when present). `Octicons` is
  an `object` of `OcticonIcon(name, small, large)`.
- **Palettes generated** by `tools/tokens/gen.py` into `Palettes.kt`.
  GitHub Desktop values are GHD 3.6.6's as `crates/corvene-ui/src/theme/*.rs`
  computes them; GHD's high contrast is dark-only, so Desktop + light + high
  contrast uses Primer's light high-contrast palette. Sponsors and "done" have
  no GHD tokens: Primer v4 pink / purple. Mobile dark text is `#e6edf3` (what
  GitHub Mobile ships) rather than Primer's current `#f0f6fc`.
- **Not yet provided**: `LocalDiffPalette` and `LocalOcticonTint` (the diff
  tokens are in `PrimerColors`; tint is an `OcticonTint` parameter).
- **Manifest**: the GPUI app's minus NativeActivity, the Vulkan/GLES features,
  `CorveneTransferService` and `CorveneDocumentsProvider` (their classes arrive
  with M-A3). Intent filters and `<queries>` unchanged. `MainActivity` keeps
  the system's default `configChanges` (Compose handles recreation).
- **Debug-only intent extra** `corvene.debug.addRepository` (see README) to add a
  repository by path in scripted tests without driving the system picker.
- **Detekt**: WMKeyboard's rule set (comments trimmed), plus
  `UnusedPrivateMember.ignoreAnnotated: Preview`. Detekt runs over main sources
  per module (`detektFossDebug`); unit-test sources are not analysed yet.

- **Cargo target dir**: `corvene.cargoTargetDir=android/build/cargo` in this
  worktree's local.properties (the shared `target/` was locked by other cargo jobs
  for 25+ minutes at a time), dev builds with `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
  (`corvene.rustDebugInfo`; full DWARF did not fit on the disk) and
  `corvene.uniffiBindgen=standalone` (`tools/uniffi-bindgen`, generator only, no host
  build of the engine). The defaults stay as the design says (workspace target,
  `cargo run -p corvene-ffi --bin uniffi-bindgen`).
- **`--config` is not passed** to uniffi-bindgen: 0.32 library mode reads
  crates/corvene-ffi/uniffi.toml through `cargo metadata` and rejects the old flat
  format on `--config`.
- **`refreshIndicators()` on every process start** from MainActivity (FFI-REQUESTS.md #9).
- **Theme**: the menu offers System / Light / Dark / High contrast (the engine's `ThemeVm`).

## Measurements (OPPO CPH2481, Android 15, fossDebug, dev-profile engine, 2026-10-04)

| What | Value |
|---|---|
| APK (foss debug, arm64-v8a) | 51.6 MB; libcorvene_ffi.so 66.9 MB unpacked (dev profile, line tables), bundled git+ssh+lfs ~39 MB unpacked |
| `am start -W` cold TotalTime | 1287, 1089, 1412, 1286, 1602, 1255, 1890, 1174, 1234, 1417 ms (median ~1.29 s; debuggable build, unoptimised Rust) |
| engine start (`Corvene(...)` incl. store open + git detect) | 179–465 ms on the ffi thread, after `System.loadLibrary` 5 ms |
| memory after start (one repo) | TOTAL PSS 123 MB, RSS 280 MB, native heap 23 MB |
| 16 KB alignment (`checkElfAlignmentFossDebug`) | every 64-bit `.so` 16384 |

Release / fast builds and the baseline profile (design targets TTID ≤ 600 ms) are not
measured yet: M-A0 ran fossDebug only.

## Verified on the phone

- `:app:installFossDebug` installs `com.wasimaster.corvene.compose` beside the GPUI app.
- `adb logcat -s corvene`: "Corvene 0.1.0-dev (foss) starting", "loaded libcorvene_ffi.so",
  engine INFO lines (store schema, `using git path=…/files/git/bin/git version=2.56.0`),
  "engine started in … ms", then `state_changed 1…n`.
- Empty store → Blankslate "No repositories yet" + Add repository (build/m-a0.png history).
- A repository pushed into `files/repositories/demo` (adb push + run-as tar) and added
  with the debug intent extra: listed under "Other"; pull-to-refresh fills branch
  `main` and the uncommitted-changes dot; persists across force-stop + cold start;
  tapping opens the Repository placeholder (branch, path).
- The SAF "Add repository" picker was not driven (it is a system activity; this
  session does not tap outside the app). `FolderResolver` is covered for `primary:`
  with all-files access and reports other providers as unsupported.

## M-A1 phone run (2026-10-04, CPH2481, debug build, GitHub Mobile style, dark)

- Cold `am start -W` TotalTime over 5 runs: 1133, 1069, 1066, 1382, 1004 ms (median 1069).
- Changes list: 4 files with status icons, include checkboxes, filter chips, count, commit panel: OK.
- Diff of `main.rs`: syntax colours (keyword/string/macro) render after the TokenClass fix
  (2513bcc8). Found and fixed on the Rust side: the hunk header showed twice and every line
  index was off by one per hunk, so the line toggle hit the wrong line (69107442, rebuilt after).
- Styles: GitHub Desktop toolbar (captions truncate to "Curr d…" at 360 dp: shorten or hide
  captions on compact), Material, GitHub Mobile. Branch chip truncates "main" to "m…" on
  compact in the Mobile/Material chrome (chip too narrow).
- Commit from the sheet: first attempt failed with the engine's "Could not commit" dialog
  (git: Author identity unknown; the fresh store has no identity): the dialog body is empty
  (the `message` field is not rendered, only "Show details"). After `git config --global
  user.*` the commit landed (`git log` shows it) and Undo reverted it with the changes staged.
  With the keyboard up the sheet moves, so the Commit button sits at y≈1218 not 2232.
- Undo bar shows HEAD's commit ("Add main.rs", made from the CLI) even before any in-app
  commit and again after Undo: check where `lastCommit*` comes from.
- Landscape (800×360 dp = medium width): the chrome (app bar + branch row + tabs + chips +
  undo bar) fills the height; no file rows visible; tapping where the list was did nothing.
  Needs a short-height layout (collapse the branch row and chips, hide the undo bar when the
  list is short), as the GPUI app's `theme::short` does.
- Error dialog from the engine (`popup()` kind Error) displays and closes.
- New FFI since the agent's run: `setGlobalIdentity(name, email)` (706be347) for Configure Git.
- Diff fling on the 6000-line `big.py` (debug build): 41-47 frames per 8 flings, 66 % janky,
  Choreographer "Skipped 61 frames". `gfxinfo framestats`: input/measure/draw tiny, the
  **animation stage (composition) p50 24 ms, p90 103 ms, max 182 ms** → a composition storm:
  ~50 new rows per fast-fling frame, each a Row of four Texts + stringResource + semantics,
  wrap mode (compact default) measuring with IntrinsicSize.Min, all under debug Compose.
  Next: measure the `fast` build; then one Text per row with Canvas-drawn gutters and a
  fixed row height in wrap-off mode, `stringResource` hoisted (done), and a baseline profile.
- **Fast build (`installFossFast`, non-debuggable, release Rust) on the same 6000-line diff:
  672 frames per 8 flings, 1.19 % janky, p90 15 ms, p95 16 ms, p99 22 ms; composition
  p50 1.5 ms, p90 3.8 ms.** The debug numbers above were debug-Compose overhead. Cold start
  (fast, empty store) 497-988 ms; with one repository 716 ms.
- The fast variant installs as `com.wasimaster.corvene` (no suffix outside debug), a separate
  store; `run-as` is unavailable, so repositories come in through the folder picker. The
  picker's `primary:` tree is accepted only with "All files access" (the SAF import copy is
  M-A3); with the permission granted in Settings, Corvene/demo2 on shared storage was added
  and shows its full path. `appops set … MANAGE_EXTERNAL_STORAGE allow` is refused on ColorOS.
- `generateUniffiRelease` failed with "No UniFFI metadata found": the workspace release
  profile has `strip = true`, and library-mode bindgen reads the static symbol table. The
  cargo task now sets `CARGO_PROFILE_RELEASE_STRIP=false` and strips itself (98c0e33b).
