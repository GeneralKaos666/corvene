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
