# What the Kotlin app needs from `crates/corvene-ffi`

Requests from the Android side; the Rust side owns the crate. Each says what
Kotlin does meanwhile.

1. ~~Repository list groups in the view model~~: done (`RepoListVm.groups`);
   Kotlin resolves the ids and filters by name (the picker's FilterField).
   Still open: a `filter` argument to `repo_list` so the fuzzy match is GHD's.
2. ~~Row icons~~: done (`RepoVm.owner/fork/private/alias`); lock and fork
   icons are drawn. The rename UI (alias) is M-A2+.
3. ~~Design style and colour mode in the settings~~: done.
4. ~~App visibility~~: done; `CorveneApp` calls `appVisible` from
   ProcessLifecycleOwner ON_START/ON_STOP and `focus()` on ON_RESUME.
5. ~~Start-up readiness~~: `ready()` exists; the splash still waits for the
   first `settings()` + `repoList()` (both answer at once now).
6. **`kotlin_target_version = "2.2.20"`** in `uniffi.toml` (named in
   design-core-host.md) so the generated code never uses newer language features
   than AGP 9.3's built-in Kotlin.
7. ~~Android-only compile error~~ (`set_bridge` wanted `Box<dyn Bridge>`,
   api.rs:88): fixed upstream. Lesson: `cargo check` on the host skips the
   `cfg(target_os = "android")` paths; run `cargo ndk -t arm64-v8a check -p corvene-ffi`
   (or `./gradlew :core:ffi:cargoNdkDebug`) after touching them.
8. ~~`CoreError::Failed { message }` broke the Kotlin bindings~~ (a field named
   `message` clashes with `Throwable.message`): renamed to `reason` upstream.
9. ~~Indicators at start-up~~: the constructor refreshes them; MainActivity
   no longer does.

## M-A1

10. **Two highlight classes are lost.** `TokenClassVm::from` matches on the
    Debug names of GHD's CodeMirror classes, but `corvene_highlight::TokenClass`
    is `Variable, AltVariable, Keyword, Atom, String, Qualifier, Type, Comment,
    Tag, Attribute, Link, Header, Quote`: `AltVariable` and `Type` fall into
    `Other` and draw uncoloured. Wanted: `AltVariable → Variable2`,
    `Type → Variable3` (Kotlin already maps those to `syntaxAltVariable` /
    `syntaxType`), or a `TokenClassVm` that mirrors `TokenClass` one to one.
11. **Selection revision in the diff header.** Toggling a line keeps
    `diff_generation`, so Kotlin re-reads the visible pages (≤ 600 rows) after
    every state change to pick up `selected`. Wanted:
    `DiffHeaderVm.selection_revision: u64` (bumps when the file's
    `DiffSelection` changes) so the pager refreshes only then.
12. **Select all.** GHD's list header checkbox (`set_files_included(repo,
    paths, include)` exists in the Dispatcher) is not exported; the header
    shows the count only.
13. **Commit draft in the engine.** The summary/description live in
    `rememberSaveable` (cleared when `commit_nonce` bumps); design §4 wants
    `update_draft(repo, summary, description)` + `CommitFormVm.summary /
    description` so a draft survives process death and repository switches
    like GHD's.
14. **Discard through the popup.** Kotlin shows its own confirmation when
    `confirm_discard_changes` is on and then calls `discard_changes`. Wanted:
    `request_discard_changes(repo, paths)` that opens
    `Popup::DiscardChanges` (or discards at once when the setting is off), so
    the prompt's "Do not show this again" writes the setting (M-A2 popups).
15. **Visible files.** `ChangesVm.files` is the whole status; Kotlin applies
    the filter options itself (`matches_options`), without the text filter,
    the `703-changes-sort-order` order or hidden paths. Wanted: either the
    files already filtered and sorted, or `ChangesVm.visible: Vec<u32>`.
16. **Gutter width.** `DiffHeaderVm.max_line_number` (old and new) to size the
    line-number gutters; Kotlin picks 36 or 44 dp from the row count.
17. **Commit author avatar.** `CommitFormVm.author` is the login; wanted the
    avatar file path (and `request_avatar(login)` when missing) for Coil.
18. **Split view rows** (`diff_rows_split(repo, generation, start, count)`
    pairing deletes with adds) for the split diff on expanded widths; the gear
    menu offers Split but the diff stays unified until then.
19. **Image diffs.** `DiffKindVm::Image` carries no files; wanted old/new blob
    paths (temporary files) to draw them with Coil.

## M-A2

20. ~~Commit diff rows~~: done (`commitDiffRows`, 5928af22); History's file
    diff is the read-only DiffScreen over them. Also used:
    `submitGenericAuth`, `retryPopupAction`, `publishRepository`.
    (`setDiffLines` is not wired yet: drag selection is M-A3+.)
21. **Upstream checks** (`rerunChecks(repo, failedOnly)`) for `CICheckRunRerun`
    and `updateUpstreamRemote(repo)` for `UpstreamAlreadyExists`: both dialogs
    only inform and close.
22. **The comparison in `HistoryVm`**: `compare: { branch, mode, ahead, behind }`.
    Kotlin keeps the compared branch and mode itself and can count only the
    mode on screen (`totalLoaded`).
23. **A typed banner.** `BannerVm.text` is the Debug form; Kotlin reads its
    fields with a regex (`bannerFields`). Wanted `fields: Vec<KeyValue>` like
    `PopupVm` (our_branch, their_branch, count, description, branch, sha).
24. **Request entry points that open GHD's popups** (respecting the
    confirmation settings and "Do not show again"):
    `requestDeleteBranch(repo, name)`, `requestRenameBranch`,
    `requestCreateBranch(repo, targetSha?, initialName)`,
    `requestCreateTag(repo, sha)`, `requestCheckoutCommit(repo, sha)`,
    `requestResetToCommit(repo, sha)`, `confirmOrForcePush(repo)`,
    `requestDropStash(repo)`, `requestUndoCommit(repo)`,
    `startRebaseFlow(repo, base?)`, `startMergeFlow(repo, squash)`.
    Meanwhile Kotlin shows the same dialog composables itself and calls the
    confirmed action.
25. **`endMco(repo)`**: dismissing the ChooseBranch step only closes the popup;
    the operation state lingers until the next one starts.
26. **`startRebase(repo, base, forcePushChecked)`**: WarnForcePush's Begin
    cannot go on (the exported call passes false and would warn again); Begin
    is disabled.
27. **`stashAndRetry()`** for `LocalChangesOverwritten` (GHD's one button);
    Kotlin offers Stash changes and Retry separately.
28. **The current branch's stash.** `ChangesVm.stashCount` /
    `BranchesVm.stashCount` count every stash, so the Changes banner says
    "stashed changes on this branch" on feature-a for main's stash (phone run).
    Wanted `stashOnCurrentBranch: bool` (GHD `desktop_stash` of the branch).
29. **`undoDeleteBranch(repo)`** for the BranchDeleted banner's Undo (flag
    861); disabled meanwhile.
30. **`discardAllAndCheckout(repo, branch)`** for StashAndSwitchBranch's
    "Discard my changes" (flag 865); not offered meanwhile.
31. **Update from default** with `pull.rebase` (flag 859): Kotlin merges the
    default branch; wanted `updateFromDefault(repo)`.
32. **`RepoVm.htmlUrl`** for View on GitHub on Enterprise hosts; Kotlin builds
    github.com URLs from `RepoVm.github`.

## M-A3

33. **`AccountVm.avatarPath`** (the file the engine downloaded, as design §4
    says) and **`AccountVm.emails`** (Configure Git's prefill, GHD picks the
    primary or noreply address). Meanwhile Kotlin downloads `avatarUrl` itself
    into `cache/avatars/` (`AvatarCache`) and Configure Git prefills the name
    only.
34. **The current global identity** (`gitIdentity(): {name, email}?`) so
    Configure Git and Settings › Git show what git already has.
35. **`HostEvents.sharePath(path)`** for the Bridge's `share_path` (GHD-less
    Share on Android), and `view_path_with(path, component, line)` /
    `view_apps()` routed to Kotlin (Settings › Integrations). `share_path`
    answers "not available" today; `OpenPath.share` is ready in
    `:core:platform`.
36. **`HostEvents.openTermux(dir)`** (the Bridge's `open_termux` /
    `run_termux` / `termux_programs`) so the engine's own "Open in Terminal"
    reaches `Termux.open`. Meanwhile the repository overflow menu calls it
    from Kotlin.
37. **`cancelTransfers()`** for the transfer notification's Stop: today Stop
    only ends the foreground service (git goes on while the process lives).
38. **`gitignoreNames()` / `licenses()`** (the bundled template names) for
    Create; Kotlin lists 21 common .gitignore templates and 14 licenses by
    the engine's names meanwhile.
39. **`HostInfo` updates at run time** (`setHostInfo` or `allFilesAccessChanged(bool)`,
    `notificationsAllowedChanged(bool)`): the record is read once at start, so
    granting "All files access" or notifications later is seen only after a
    restart.
40. **`endHeadless()` that waits** for the headless fetch to return (the GPUI
    app's `nativeEndHeadless` did); now it only cancels, so `Corvene(...)` may
    start while the headless git is still being stopped.
41. **A `remoteBusy`/`backgroundFetch(): bool`** answer so the worker can stay
    until the live fetch ended (design §4: await busy == false, 2 min cap);
    `backgroundFetch()` is dispatched and the worker returns at once.

## M-A5

42. **`SettingsVm.confirmDiscardChangesPermanently` and `useExternalCredentialHelper`.**
    `setSetting` writes both keys but the view model does not carry them, so
    Settings › Prompts has no "Discarding changes permanently" row and
    Advanced no "Use Git Credential Manager" row (a switch needs its value).
43. **The global git config without the Preferences popup.** `globalGitConfig()`
    is filled only by `Dispatcher::open_preferences`, which the FFI does not
    export, so it answers null; Settings › Git shows the default branch as
    "main" (read-only). Wanted `loadGlobalGitConfig()` (or `openPreferences(tab)`)
    and **`setDefaultBranch(name)`** (GHD's Default branch tab writes
    `init.defaultBranch`).
44. **`HostEvents.relaunch()`** apart from `quit()`: the Bridge's `relaunch`
    calls `events.quit()`, so Kotlin treats every Quit as "start again"
    (RelaunchActivity). A real quit (none on Android today) would restart.

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

## Provided by the FFI (2026-10-04, 5928af22 + d3b5263e + later)

- `commitDiffRows(repo, generation, start, count)`, `setDiffLines(repo, path, from, len, selected)`,
  `submitGenericAuth(username, password)`, `retryPopupAction()`, `stashAndRetry()`,
  `publishRepository(repo, name, description, private, endpoint, org?)`, `rerunChecks(failedOnly)`,
  `updateUpstreamRemote(repo, update)`.
- Dialog entry points (GHD's own popups, confirmation settings respected): `requestDeleteBranch`,
  `requestRenameBranch`, `requestCreateBranch(repo, targetSha?, initialName)`, `requestCreateTag`,
  `requestMerge(repo, squash)`, `requestPublishRepository`, `requestCheckoutCommit`,
  `requestResetToCommit`, `confirmOrForcePush`, `requestDropStash`, `requestUndoCommit`,
  `requestDiscardChanges(repo, paths)`, `requestRemoveRepository`, `startRebaseFlow(repo, base?)`,
  `startRebaseWith(repo, base, forcePushChecked)`, `endMco`, `requestAbortMco`,
  `undoDeleteBranch()`, `discardAllAndCheckout(repo, branch)`, `updateFromDefault(repo)`.
- `HistoryVm.compare: CompareVm? {branch, mode, ahead, behind, mergeStatus, mergeConflicts}`,
  `BannerVm.repo/fields` (typed key/values like PopupVm), `ChangesVm/BranchesVm.stashOnCurrentBranch`,
  `RepoVm.htmlUrl`.
- Settings + Flags (68baf3d0): `SettingsVm` now also has the confirm_* booleans, showCommitLengthWarning,
  commitSpellcheckEnabled, historyFirstParent, uncommittedChangesStrategy ("ask"/"stash"/"move"),
  externalEditor, shell, cloneDir; `setSetting(key, value)` (snake_case key, "true"/"false" or text);
  `suspend flags(): FlagsVm` (preset, presets, categories, flags[ident, slug, id, title, summary,
  ghdBehaviour, category, nature, kind, options, min/max/unit, value, valueLabel, isOn, overridden,
  presetValue, ghdValue, restart, restartPending, available]), `setFlagBySlug(slug, text)` (throws
  CoreException with the reason), `resetFlag(slug)`, `applyPreset("github-desktop"|"familiar"|"corvene"|"max")`,
  `resetAllFlags()`, `relaunch()`.
- Repository settings (c1a405bf): `openRepositorySettings(repo, "remote"|"ignored"|"git")`,
  `suspend repositorySettings(): RepositorySettingsVm?`, `saveRepositorySettings(repo, remoteName?,
  remoteUrl?, gitignore?, gitConfigLocation?, name?, email?, autocrlf?)`, `openGlobalGitConfig()`,
  `suspend globalGitConfig(): GlobalGitConfigVm?`.
- M-A3 requests (next commit after a7e8441a): `AccountVm.avatarPath` + `emails`; `suspend gitIdentity(): GitIdentityVm?`;
  `gitignoreNames()`, `licenses(): List<LicenseVm{name, featured, hidden}>`; `setHostInfo(HostInfo)` (live
  all-files/notification state); `endHeadless()` now waits (≤ 10 s) for the headless fetch to return;
  `networkBusy(): Boolean` for the worker; `cancelTransfers()` (stops the clone; a cancel-all of git
  network commands needs core work). **Breaking:** `HostEvents` gained `sharePath(path)`, `openTermux(dir)`,
  `runTermux(program, arguments, dir)`, `viewPathWith(path, component, line?)`, `viewApps(): List<String>`
  ("label\tpackage/class" per entry) and `packageInstalled(package): Boolean` — `HostEventsBridge` must
  implement them (route to OpenPath.share, Termux.open/run, OpenPath.viewWith, the installed viewers probe,
  PackageManager). The engine's own Share / Open in Termux / Open with now reach Kotlin through them.

## Provided by the FFI (2026-10-04, M-A5 requests 42-44)

- `SettingsVm.confirmDiscardChangesPermanently` and `useExternalCredentialHelper` (#42); the Prompts
  and Advanced screens show their rows.
- `loadGlobalGitConfig()` reads the global identity and `init.defaultBranch` into `globalGitConfig()`
  without the Preferences popup; `setDefaultBranch(name)` writes `init.defaultBranch` and re-reads it
  (an error dialog on failure) (#43). Settings › Git has GHD's main / master / Other… radios.
- `HostEvents.relaunch()` (#44), raised by the engine's own relaunch (a flag that needs a restart);
  `quit()` is now a real quit. **Breaking:** `HostEventsBridge` implements it (`HostRequest.Relaunch`
  → RelaunchActivity; `HostRequest.Quit` → `finishAffinity`).
