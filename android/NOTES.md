# M-A0 notes

Status, measurements and every place this build differs from
`.docs/android/design-compose-app.md`.

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
