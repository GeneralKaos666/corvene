# Notes

Status, measurements and every place this build differs from
`.docs/android/design-compose-app.md`. Newest milestone first.

# M-A3 notes

## Built

- `:core:ffi`: `NativeContext.attach` (JNI `Java_com_wasimaster_corvene_ffi_NativeContext_attach`)
  behind `Native.prepare(context)` (load + attach once), called by `Core`'s
  start-up before `Corvene(...)` and by `Headless.fetch` before
  `headlessFetch`; `Headless.end()` (`endHeadless`, only once the library is
  loaded); `Core.start(context, config)`, `Core.isRunning` / `Core.current`.
- `:core:platform`: `FolderResolver` (own provider → its file; `primary:` +
  all-files access → `/storage/emulated/0/…`; else a tree holding `.git` is
  copied into `files/repositories/<name>[-n]` with an "Importing…" toast, else
  "not a Git repository" / "allow All files access"; `import = false` for
  destinations), `FileCopier` (single files → `cache/tmp/picked/file`, 16 MB
  cap), `rememberFolderPicker(import)` / `rememberFilePicker` (each answer
  exactly once), `CorveneDocumentsProvider` (Kotlin port: roots `repositories`
  + `home`, `%2E` dot ids, hidden `shared/` `tmp/`, create/delete/rename,
  folders-first sorting, `findDocumentPath`), `OpenPath` (view with chooser,
  reveal = the parent folder, share), `Notifications` (channels
  `pull-requests` + `transfers` at start, pull request notifications whose tap
  → `notificationClicked`), `TransferService` (dataSync, Stop action,
  `onTimeout`) + `TransferController` (starts after 1.5 s of transfer),
  `CorveneFetchWorker` (CoroutineWorker, 1 h, CONNECTED + battery not low,
  UPDATE, 1 h initial delay; live engine → `backgroundFetch()`, none →
  headless; FAILED → retry), `Termux.open` (RUN_COMMAND, shared storage only,
  asks the permission), Custom Tabs for GitHub's `/login/oauth/authorize` and
  `/login/device` pages, `AppLinks` (VIEW x-corvene / x-corvene-auth as is,
  GitHub/remote URL or SEND text with an address → `x-corvene://openRepo/<url>`),
  `AvatarCache` (https avatar → `cache/avatars/`), HostRequestHandler for every
  request (pick folder/file, open path, notification + permission, all-files
  access, transfer, bring to front).
- `:feature:onboarding` (`onb_`): WelcomeScreen (Start → Sign in → Configure
  Git, step dots), SignInPanel (browser button = `signIn(null)`; steps
  Requesting / DeviceCode with Copy + Open github.com + spinner / Browser /
  Verifying / Error + Try again; Enterprise host + token →
  `signInWithToken`; signed-in row with avatar), WelcomeRoute (a new account
  moves on to Configure Git; Finish = `setGlobalIdentity` + `completeWelcome`,
  Skip = `completeWelcome`), SignInRoute (Settings).
- `:feature:repositories`: CloneScreen (GitHub.com tab: `loadCloneableRepositories`
  / `cloneableRepositories`, filter, pick; signed out: "Sign in to list your
  repositories"; URL tab: URL or owner/name), local path row (default
  `settings().cloneDir`, else `files/repositories/<name>`; Choose… picks a
  destination), Shallow clone (`depth` 1); AddRepositoryScreen (folder via the
  importing picker; "Use folders in place" Flash → all-files settings where
  it can be asked for); CreateRepositoryScreen (name with GHD's sanitising,
  description, folder, README, .gitignore and license menus);
  CloneProgressRoute (`session().cloning` dialog, Cancel = `cancelClone`; the
  added repository opens). Empty list Blankslate: Clone / Create / Add.
- `:feature:settings`: AccountsScreen + route (avatars, Sign out with a
  confirmation → `signOut`, sign-in rows).
- `:app`: Welcome replaces the navigation while `welcomeCompleted` is false;
  keys Clone(url?) / AddRepository / CreateRepository / SignIn(enterprise) /
  AccountsSettings; "+" ActionMenu on the list; overflow: Accounts, and on a
  repository Show in Files + Open in Termux (when installed); PopupHost
  renders the engine's CloneRepository / CloneRepositoryRetry /
  AddExistingRepository / CreateRepository as full-screen dialogs and turns
  SignIn into the destination; MainActivity: `Headless.end()` then the engine,
  intents through `AppLinks`, notification taps → `notificationClicked`.
  Manifest: TransferService, CorveneDocumentsProvider
  (`${applicationId}.documents`, MANAGE_DOCUMENTS, DOCUMENTS_PROVIDER),
  `dataExtractionRules` (everything excluded); foss keeps
  MANAGE_EXTERNAL_STORAGE.

## Deviations (and why)

- **Engine start moved from `CorveneApp.onCreate` to `MainActivity.onCreate`**:
  CorveneApp runs in the process WorkManager starts for the fetch too, and an
  engine there would make the headless path unreachable. The activity starts
  a few ms after the application, so start-up is unchanged in practice.
- **Import copies with one cursor per folder** (DocumentsContract), not
  DocumentFile: DocumentFile queries the provider again for every name and
  type, ~3 IPCs per file of a `.git`.
- **Shorthand `owner/name`** in the URL tab becomes `https://github.com/owner/name`
  in Kotlin (GHD resolves it through the API).
- **Templates**: 21 common .gitignore names and 14 licenses (engine names),
  not the full bundled lists (FFI-REQUESTS #38).
- **Avatars** downloaded by Kotlin (`AvatarCache`), not the engine (#33).
- **Configure Git** prefills the account's name only; no emails in the view
  model (#33/#34).
- **Transfer Stop** ends the service only (#37). The service starts 1.5 s
  into a transfer while the app is in front (as the GPUI app); Android 12+
  refuses a start from the background, which is logged.
- **Termux** only for repositories on shared storage (Termux cannot read app
  storage), as the GPUI app; the editors probe is M-A5 (Settings ›
  Integrations).
- **Clone tabs**: GitHub.com and URL; Enterprise accounts' lists come with
  M-A5's accounts work.
- **Worker** dispatches `backgroundFetch()` and returns at once (#41).

## Verification (2026-10-04)

- `:app:assembleFossDebug :app:assemblePlayDebug -Pcorvene.abis=arm64-v8a`: OK
  (57.3 MB each). `aapt2 dump permissions`: MANAGE_EXTERNAL_STORAGE in foss
  only; WorkManager adds WAKE_LOCK, RECEIVE_BOOT_COMPLETED and its
  DYNAMIC_RECEIVER_NOT_EXPORTED permission to both.
- New tests: FolderResolverTest 6 (own provider, primary with access, import
  copy of a fixture `.git`, `-2` suffix, no `.git`, destination never
  imported; a fake external-storage DocumentsProvider over a temp folder),
  CorveneDocumentsProviderTest 8, CorveneFetchWorkerTest 2 (work-testing),
  AppLinksTest 6, WelcomeScreenTest 7, RepositoryFormsTest 6,
  AccountsScreenTest 2; screenshots welcome ×4 steps (start, sign in, device
  code, configure git) and clone (URL, GitHub.com), create, add, each 3 styles
  × light/dark.
- `unitTests staticAnalysis verifyRoborazziFossDebug`: green (one rerun of
  `:feature:onboarding:testFossDebugUnitTest` after a corrupted binary test
  result left by the full disk; its start screenshots were re-recorded after
  the body text changed).
- Disk: filled twice (Gradle's transforms cache was lost with it and rebuilt;
  `build/cargo/debug/{deps,build}` deleted).

## Phone checks owed (the phone dropped off wireless adb for the whole run)

Install `:app:installFossDebug` (`com.wasimaster.corvene.compose`), then:
(a) Welcome does not reappear for the existing store; Welcome on `pm clear
com.wasimaster.corvene` (fast variant, rebuilt with `installFossFast`);
(b) Sign in with browser → device code (no client secret in dev builds) →
"Open github.com" Custom Tab; the user enters the code; then
`session().accounts` non-empty after `am force-stop` (Keystore via
NativeContext); (c) clone `https://github.com/octocat/Hello-World.git` →
progress dialog → repository opens, History has commits; (d) Add existing:
fast variant `/sdcard/Corvene/demo2` in place, debug variant without
all-files access → import into `files/repositories/demo2`; (e) `am start -a
android.intent.action.VIEW -d x-corvene://openRepo/octocat/Spoon-Knife -p
com.wasimaster.corvene.compose` → Clone prefilled; (f) `ACTION_SEND` text →
Clone prefilled; (g) Files root via `content://com.wasimaster.corvene.compose.documents/root/repositories`;
(h) `cmd jobscheduler run -f com.wasimaster.corvene.compose <id>` with the app
killed → logcat `headless background fetch`, then open the app; (i) Open in
Termux on a shared-storage repository; (j) `android/build/m-a3-*.png`.

# M-A2 notes

## Built

- `:feature:branches` (`br_`): BranchSheet (SelectPanel: Branches | Pull requests
  tabs for GitHub repositories, filter, "New branch", groups Default / Recent /
  Other, remote branches without a local twin folded under "Remote branches",
  current branch checked, relative tip time; tap = `checkoutBranch(…, null)`,
  long press = menu: Rename, Merge <b> into <current>, Rebase <current> onto <b>,
  Update from <default> (on the current branch), Compare in History, Delete),
  pull request rows (draft/open StateLabel, Checkout = `checkoutPullRequest`),
  BranchGroups + `sanitizeBranchName`, dialogs CreateBranch (default vs current
  choice, from a commit), RenameBranch, DeleteBranch (+ remote switch),
  StashAndSwitchBranch, ConfirmOverwriteStash, ConfirmSwitchBranch, sync:
  `syncButtonModel` (Fetch origin · Last fetched, Pull n↓, Push n↑, Publish
  branch / repository, the running operation with progress), `SyncMenu` (long
  press: Fetch, Pull, Push / Publish branch, Force push with lease…),
  `SyncController` (tap = the next step; force push asks first when
  `confirmForcePush`), ConfirmForcePush, PublishRepository (accounts from
  `session()` → `publishRepository`), GenericGitAuthentication (→
  `submitGenericAuth`).
- `:feature:history` (`hist_`): HistoryScreen (CommitPager: 100-commit windows of
  `history(repo, start, count)`, LRU of 10, refresh after state changes,
  `loadMoreCommits` 20 rows before the end once per count; rows avatar initials,
  summary, author · relative time, tag Labels, merge icon; compare chip →
  SelectPanel → `compareToBranch`, Behind | Ahead SegmentedControl, "Merge <b>
  into <current>"; long press → menu: Select multiple, Revert, Cherry-pick…,
  Create branch from commit, Create tag…, Checkout commit, Reset to commit…, Copy
  SHA, View on GitHub (GitHub repos); multi-select mode: taps select the
  contiguous range from the anchor → `selectCommits`, bar with Cherry-pick… and
  Done), CommitDetailScreen (summary, body behind Show more, author + time, short
  SHA + copy, n changed files +adds −dels, file rows with status icons →
  `selectCommitFile`), CreateTag / ConfirmCheckoutCommit / WarningBeforeReset
  dialogs.
- `:feature:changes`: `CommitDiffRoute` (DiffScreen `readOnly` over
  `commitDiffRows`: no include box, no line toggles, History's hide-whitespace).
  Short windows: chips behind a filter button in the list header, Undo bar hidden
  unless the list is empty.
- `:feature:mco` (`mco_`): ConflictsScreen (per file Use ours | Use theirs
  SegmentedControl → `setManualResolution`, the chosen side again unresolves,
  Open in editor disabled, Abort / Continue), MultiCommitOperationRoute
  (ChooseBranch sheet for rebase / cherry-pick + new branch, WarnForcePush,
  non-dismissable progress, conflicts full screen (close = `hideConflicts`),
  ConfirmAbort (Abort asks only when something was resolved)), ChooseBranchSheet,
  NewBranchNameDialog.
- `:app`: PopupHost → stateless `PopupDialog(popup, context, actions)` (one `when`,
  Corvene's variant names plus GHD's where they differ) for Error,
  DiscardChanges, DeleteBranch/DeleteRemoteBranch, RenameBranch, CreateBranch,
  CreateTag, ConfirmForcePush, PushNeedsPull, UpstreamAlreadyExists,
  StashAndSwitchBranch, ConfirmOverwriteStash, ConfirmSwitchBranch,
  ConfirmDiscardStash, CheckoutCommit, WarnLocalChangesBeforeUndo,
  ResetToCommit, UnknownAuthors, LocalChangesOverwritten,
  ConfirmRemoveRepository, ExternalEditorError, ShellError,
  CommitConflictsWarning, HookFailed, MergeBranch, MultiCommitOperation,
  CICheckRunRerun, PullRequestChecksFailed, GenericGitAuthentication,
  PublishRepository; anything else the generic dialog. `CorePopupActions` =
  `closePopup` then the action, as GHD's dialogs. BannerFlash under the tabs
  (all `Banner` kinds, View conflicts → `showConflicts`, dismissed per nonce).
  CorveneScaffold: branch sheet, sync controller (tap / long press / dialogs),
  History tab live (compact: list → CommitDetail → CommitDiff destinations;
  medium/expanded: list | commit files over the file diff), Create branch from
  a commit.
- `:core:design`: TopChrome (sync button with long press, progress line and an
  anchor; branch chip measured first so `main` never truncates, the sync label
  gives way; Desktop compact drops the captions and the dropdown arrows and
  narrows the sync button; height < 480 dp folds everything into one 48 dp bar),
  `isShortHeight()`, SelectPanel `titleActions` / `tabs` / `inline`,
  PrimerDialog `dismissible`, PrimerTextField `password`, SegmentedControl
  `fill`, 6 Octicons (tag, git-compare, git-pull-request-draft/-closed,
  repo-push, versions; gen.py wraps lines over 140 columns).
- `:core:common` `relativeTime`, `:core:platform` `writeClipboard` / `openUrl`.

## Deviations (and why)

- **Dialogs opened from Kotlin**: the FFI exports the confirmed actions only
  (`deleteBranch`, `checkoutCommit`, `resetToCommit`, `push(force)`, …), not
  GHD's request entry points that open the popup. The branch panel and the
  commit menu show the same dialog composables PopupHost uses and then call the
  action (FFI-REQUESTS #24). Checkout commit and reset always confirm; force
  push follows `confirmForcePush`.
- **Long press on a commit** opens the menu; its "Select multiple" starts range
  selection (GHD: shift/cmd-click). Non-contiguous selection is not offered.
- **Compare counts**: the line under Behind | Ahead counts the commits of the
  mode shown (`totalLoaded`); the other mode's count needs the comparison in
  the view model (FFI-REQUESTS #22). The comparison branch/mode is Kotlin state.
- **PushNeedsPull** offers Pull (primary) and Fetch (GHD has Fetch only).
- **LocalChangesOverwritten**: "Stash changes" and "Retry" are two buttons
  (GHD's single "Stash changes and continue" needs `stashAndRetry`, #27).
- **WarnForcePush**: Begin is disabled (`startRebase` cannot pass
  force_push_checked, so it would warn again, #26).
- **Closing ChooseBranch** sends `closePopup` only; the engine's operation state
  stays until the next one starts (`endMco`, #25).
- **Banner fields** are parsed from the Debug text (`bannerFields`, #23);
  BranchDeleted's Undo is disabled (#29).
- **Branch rows**: remote branches whose local twin exists are hidden (GHD);
  the rest sit in a collapsed "Remote branches" group (Corvene).
- **Update from default** = `mergeBranch(default)`; GHD rebases instead when
  `pull.rebase` is set (flag 859, not exposed).
- **Copy SHA** writes the clipboard from Kotlin; the toast only below Android 13
  (13+ shows its own confirmation).
- **View on GitHub** builds `https://github.com/<owner/name>/commit/<sha>`:
  Enterprise hosts need the repository's html URL (#34).
- **Tests in :app**: `corvene.screenshots` applied to :app; Robolectric runs
  with `@Config(application = Application::class)` so the engine never loads.
  Dialog screenshots use `captureScreenRoboImage` (dialogs are windows).

## Verification (2026-10-04)

- `:app:assembleFossDebug -Pcorvene.abis=arm64-v8a`: OK (52 MB APK).
- `unitTests staticAnalysis verifyRoborazziFossDebug`: BUILD SUCCESSFUL, 177
  tests (new: BranchSheetTest 8, StashAndSwitchBranchDialogTest, BranchGroupsTest 3,
  HistoryScreenTest 6, CommitDetailScreenTest, CommitPagerTest 3,
  ConflictsScreenTest 3, ChangesShortHeightTest, PopupDialogTest 28 kinds,
  BannerFieldsTest; screenshots: branch_sheet ×6, history ×6 + compare +
  expanded, commit_detail ×6, conflicts ×6, popup stash/delete/discard ×6 each).
- Disk: the shared disk filled during the work (117 MB free): the host bindgen
  build under `build/cargo/debug` and old app/ffi intermediates were deleted;
  Gradle's file lock was left held by a wedged daemon and had to be killed.
  `CARGO_INCREMENTAL=0` keeps the Android dev build's incremental cache off the disk.

## M-A2 phone run (2026-10-04, CPH2481, fossDebug, GitHub Mobile style, dark)

- Cold `am start -W` TotalTime: 1365, 1137, 1140, 1181, 1210 ms (median 1181).
- Seed (`run-as`, git from the new nativeLibraryDir: the `files/git/bin`
  symlinks still pointed at the previous install's path until the app starts,
  so the script repoints them and sets `GIT_EXEC_PATH`): feature-a (2 commits),
  feature-b and main each changing `conflict.txt`, tag v0.1.0 on the first commit.
- Branch chip shows `main` / `topic-from-phone` in full at 360 dp; the sync
  button truncates first ("Publish re…").
- Checkout feature-a with 5 changes → StashAndSwitchBranch → "Leave my changes on
  main" → `git branch --show-current` = feature-a, `stash@{0}: On main:
  !!GitHub_Desktop<main>` (m-a2-stash-prompt, m-a2-after-switch).
- New branch from the sheet: "topic from phone" → caption "Will be created as
  topic-from-phone" → created and checked out (m-a2-create-typed).
- History list, commit detail, Copy SHA (system clipboard chip), the commit's
  file diff over `commitDiffRows`, commit menu, compare Behind/Ahead + merge
  button (m-a2-history, -commit-detail, -commit-diff, -commit-menu,
  -compare-behind, -compare-ahead).
- On main: long press feature-a → "Merge feature-a into main" → banner
  "Successfully merged feature-a into main", `git log` shows the merge commit
  (m-a2-branch-menu, m-a2-merged).
- On feature-b: long press main → "Rebase feature-b onto main" → conflicts screen
  (conflict.txt) → Use ours → Continue rebase → banner "Successfully rebased
  feature-b onto main"; `git log`: feature-b = main's tip (the commit became
  empty and was dropped), status clean (m-a2-rebase-result, -resolved-ours,
  -rebase-done).
- Stash Restore on main brings the 5 files back (m-a2-restored).
- Landscape (800×360 dp): one 48 dp bar (back, demo ▾, main chip, Publish
  repository, ⋯), tabs, list header with the filter button, 2 file rows, commit
  bar; no Undo bar (m-a2-landscape-changes). Rotation restored to the user's lock.
- Branch sheet in all three styles (m-a2-branch-sheet-mobile, -desktop,
  -material). Desktop at 360 dp truncated `main` to "m…" in the three-button
  toolbar: fixed after the run (compact drops the dropdown arrows, sync button
  0.8 weight); not re-checked on the phone.
- Found: the Changes stash banner ("You have stashed changes on this branch")
  shows on feature-a for main's stash (`ChangesVm.stashCount` counts every
  stash, #28).
- Not run on the phone: sync actions (the demo has no remote), the fast variant.

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
