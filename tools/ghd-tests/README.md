# GitHub Desktop test port

Corvene ports GitHub Desktop's unit tests (`app/test` of desktop/desktop,
currently release 3.6.6) to Rust, so Corvene is checked against the
behaviour GitHub Desktop itself tests. This directory tracks which upstream
cases are ported, ignored or skipped:

- `extract.py` lists every `it()` / `test()` of GitHub Desktop's `app/test`
  into `upstream.tsv` (`<file>\t<describe › … › case>\t<line>`).
- `check.py` matches that list against the `// GHD:` markers in
  `crates/**/*.rs` and the skip lists in `skips/*.tsv`.
- `crates/corvene-test-support` holds GitHub Desktop's fixtures (byte for
  byte), the isolated test environment, the ports of `app/test/helpers`
  and the helpers many test files share (see [Shared helpers](#shared-helpers)).
  Its crate docs map every GitHub Desktop helper to its Rust name and list
  the store-level helpers that are not ported there.

## Running

```sh
cargo test --workspace --test ghd              # every crate's ported tests
cargo test --workspace --test ghd -- git_status     # one module (a name filter)
cargo test --workspace --test ghd -- --ignored      # watch ignored cases fail
cargo test --workspace --test support          # the support crate's own checks

python3 tools/ghd-tests/check.py               # summary; exit 1 if any case is unaccounted for
python3 tools/ghd-tests/check.py --missing     # cases neither ported nor skipped
python3 tools/ghd-tests/check.py --ignored     # ported-but-ignored cases with reasons
python3 tools/ghd-tests/check.py --file unit/git/status-test.ts   # one file, per case
python3 tools/ghd-tests/check.py --write       # also write coverage.tsv
```

`check.py` also reports errors for markers that name no upstream case,
markers not directly above a test `fn`, ignore and skip reasons without a
known kind, skips of unknown cases or files, and cases that are both
ported and skipped. While the port is under way the
only acceptable non-zero exit is "N missing".

## Layout

Ported tests are integration tests (`tests/`) of the crate that has the
Corvene code under test (`corvene-git`, `corvene-core`, `corvene-github`,
`corvene-platform`, `corvene-store`, `corvene-ui`, `corvene-models`; each
has `corvene-test-support` as a dev-dependency). Each of those crates has
one test target, `ghd`: `crates/<crate>/tests/ghd/main.rs`, which holds
only `mod <name>;` lines (in alphabetical order), one per module file next
to it. Never use `corvene-test-support` from a `#[cfg(test)]` module in
`src/`: it depends on `corvene-git` and `corvene-models`, so inside those
crates' unit tests its types come from a second copy of the crate and do
not match.

- One GitHub Desktop test file becomes one module file
  `crates/<crate>/tests/ghd/<name>.rs`. `<name>` is the path below `unit/`
  with `/` and `-` turned into `_` and `-test.ts` / `-test.tsx` dropped:
  `unit/git/status-test.ts` → `git_status.rs`, `unit/diff-parser-test.ts`
  → `diff_parser.rs`, `unit/stores/git-store-test.ts` →
  `stores_git_store.rs`. The one name that is a Rust keyword gets a
  trailing `_`: `unit/enum-test.ts` → `enum_.rs`.
- A new module file needs its `mod <name>;` line in that crate's
  `tests/ghd/main.rs`; the module files are not targets of their own.

### Shared helpers

Look in `corvene-test-support` before writing a helper, and move a helper
there as soon as a second module needs it (when it needs nothing but
`corvene-git` and `corvene-models`; see below), with a doc comment naming
the GitHub Desktop function or helper it mirrors:

- `app/test/helpers`: `setup_fixture_repository`, `setup_empty_repository`
  and the other repository builders, `make_commit`, `get_status_or_throw`,
  `get_branch_or_error`, `git_hub_repo_fixture` (with
  `GitHubRepoFixtureOptions`, built like GitHub Desktop's object literal:
  `GitHubRepoFixtureOptions { owner: "desktop", name: "desktop",
  ..Default::default() }`)…
- `lib/git` functions tests call for setup and checks: `get_branches(path,
  &prefixes)` (`[]` for a directory that is not a repository),
  `create_commit` (fails the test) / `try_create_commit` (returns the
  `Result`, takes `CommitOptions`), `get_commits`, `get_commit`,
  `git_error_message` (`GitError.message`), `get_working_directory_diff`,
  `load_tip` (`GitStore.tip` after `loadStatus()`), `create_bare_upstream`.
- models: `new_repository(path, id, git_hub_repository)`,
  `working_directory_file_change(path, kind, selection)` (porcelain
  columns left neutral; set the ones the code under test reads),
  `from_files`, `conflicted_count`, `ghd_text` (`DiffLine.text`),
  `unified_diff_end`.
- Node and JavaScript: `write_file`, `append_file`, `base64`, `date_parse`
  (`Date.parse` of a UTC timestamp), `to_iso_string`.
- HTTP: `serve(StubResponse::new(status, body))` answers every request
  with that response and returns the base URL (`with_status_text`,
  `with_header` for the rest of GitHub Desktop's `new Response(…)`);
  `serve_with(|request| …)` answers per request (route on
  `request.path()`) and records the requests (`server.requests()`,
  `server.request_heads()`); `unreachable_endpoint()` is a URL nothing
  listens on (a stub that throws).
- `has_git_lfs()`: whether the test git runs `git lfs`.

`corvene-test-support` depends on `corvene-git` and `corvene-models` only
(never `corvene-core`: that would pull GPUI into every `corvene-git` test
binary). Helpers that need another crate's types live in that crate's
tests: `tests/ghd/<topic>_support.rs` when several modules share them,
declared in `main.rs` like any module and used as
`crate::<topic>_support::…`; a helper only one module needs stays in that
module. The flags of the `github-desktop` preset are
`corvene_core::flags::github_desktop_flags()` (and
`github_desktop_overrides()` for a hand-built `AppState`).

## Writing a ported test

```rust
// GHD: unit/git/add-test.ts › git/add › addConflictedFile › stages a conflicted file after manual resolution
#[test]
fn stages_a_conflicted_file_after_manual_resolution() {
    let repo = setup_conflicted_repo();
    let before_status = get_status_or_throw(&repo);
    // …
}
```

- **Marker**: a line `// GHD: <file> › <case path>` directly above the
  test's attributes, with the file and the case path exactly as in
  `upstream.tsv` (the separator is ` › `, U+203A). A test may carry several
  markers; a case may be marked on several tests (it counts as ignored if
  any of them is). Only attributes and comments may stand between the
  marker and the `fn`, and one of the attributes is the test attribute
  (`#[test]`, `#[gpui::test]`…).
- **Name**: the snake case of the case title, shortened when huge; prefix
  part of the `describe` path when two cases of a file share a title.
- **Faithful**: same setup, same steps, same assertions and messages. The
  `t` (`TestContext`) argument disappears (temporary directories are removed
  when their guard drops), `await` disappears (everything is blocking), and
  GitHub Desktop's `Repository` is a `TestRepo` (`repo.path()`).
  `corvene_git` functions take `git()` and a `&Path`. A `beforeEach` /
  `before` becomes a setup function each test of that `describe` calls
  first (tests share no state); an `afterEach` becomes a guard's `Drop` or
  disappears with the temporary directories. Never weaken an assertion to
  make a test pass.
- **Corvene equivalents**: say in the module doc which Corvene function
  stands for each GitHub Desktop function under test, and why when it is
  not obvious (see `crates/corvene-git/tests/ghd/git_init.rs`). When there
  is none, write a stand-in named like the GitHub Desktop function that
  calls `unimplemented!()`, keep the faithful test and ignore it with
  `ghd: missing:` (see `git_description.rs`).
- **git**: run git with `exec` / `exec_with` (dugite's `exec`: any exit
  code is returned, never a failure), `exec_ok` (GitHub Desktop's `git()`:
  fails unless exit code 0) or `git_command(cwd)` (a
  `corvene_git::GitCommand`). Never spawn `git` with
  `std::process::Command`.
- **Environment**: every test binary that uses `corvene-test-support` runs
  with `HOME` in an empty temporary directory (and no `XDG_CONFIG_HOME`, so
  the XDG global config is `$HOME/.config/git/config`), no system config,
  no inherited `GIT_*` variables, `EDITOR` or `VISUAL`, `TERM=dumb` (as
  GitHub Desktop's `git()` sets) and the identity `Joe Bloggs
  <joe.bloggs@somewhere.com>`, set up before `main`. Call
  `init()` first in a test that runs git (or `gix`) without any helper.
  Never call `std::env::set_var` / `remove_var` in a test: tests run on
  parallel threads. Pass per-command variables with `exec_with(..,
  ExecOptions { env, .. })` or `git_command(..).env(..)`; GitHub Desktop's
  `env: { HOME }` (a private global config for one call) becomes
  `env: vec![("HOME".into(), home.path().into())]`. A GitHub Desktop test
  that sets `process.env` around a call (`GIT_CONFIG_PARAMETERS`, …) runs
  that part inside `corvene_git::process::with_env(&[(name, value)], || …)`,
  which gives the variables to every git command started on the test's
  thread until the closure returns (see `git_clone.rs`); code that runs git
  on other threads, or reads the variables itself, cannot be ported that
  way: ignore it with `ghd: env:`. The proxy variables are cleared and
  `NO_PROXY` covers `127.0.0.1`, `localhost` and `::1`, so the stub servers
  are reached directly. Never turn on `corvene_git::hook_env`
  (`set_hook_env`, or the Git hooks environment setting in a store test):
  it copies the login shell's `HOME` and `GIT_*` into every git process of
  the binary.
- **Global config**: `$HOME/.gitconfig` is shared by every test of a
  binary. A test that writes it (`git config --global`,
  `set_global_config_value`, `set_default_branch`, `add_safe_directory`…)
  or depends on it being empty (`configured_default_branch`) holds `let
  _global = lock_global_config();` for its whole body; the file (and
  `$HOME/.config/git`) is empty when the guard is handed out and after it
  drops. The other tests still read it meanwhile, so prefer a per-call
  `HOME` or `with_env` (above) when the code under test takes one or runs
  git on the test's thread. Never delete or rewrite those files in place
  yourself: git dies when a configuration file it saw a moment ago is gone
  (the guard empties them with an atomic `rename`).
- **Fixtures** are read-only: use `setup_fixture_repository("name")` (a
  temporary copy with every `_git` renamed to `.git`) and
  `get_fixture_path([..])` only to read files.
- **Paths**: temporary paths are not canonical (on macOS `/var/folders/…`
  is `/private/var/folders/…`), exactly as in GitHub Desktop; canonicalize
  before comparing with paths git prints.
- **Platform cases**: a case GitHub Desktop only runs on one platform gets
  `#[cfg(windows)]` (or similar) on the test fn; it still counts as ported.
  A case that cannot pass on one platform only gets
  `#[cfg_attr(<cfg>, ignore = "ghd: <kind>: <why>")]`; `check.py` counts it
  as ignored.

### Ignored cases

A case that cannot pass yet keeps its faithful assertions and gets

```rust
#[ignore = "ghd: <kind>: <why>"]
```

between the marker and the `fn`, on one line (`check.py` reads it and
reports any other form as an error). Kinds:

- `bug`: Corvene behaves differently and should not (fix Corvene, then drop
  the ignore),
- `missing`: Corvene has no equivalent API yet,
- `todo`: a `.docs/TODO.md` item (name it),
- `deviation`: a documented `.docs/deviations.md` difference; cite its
  flag when it has one (`ghd: deviation: 123-some-flag: …`),
- `env`: needs something the test environment cannot provide (network,
  credentials, a GUI, another platform's tools).

## Skips

Cases that are not ported at all are listed in `skips/*.tsv` (one file per
topic, so parallel work does not edit the same file):

```
# comment
unit/copilot-byok-test.ts	*	omitted: Copilot is left out by design
unit/some-test.ts	describe › case	n/a: tests a React component's props
```

Three tab-separated columns: the file as in `upstream.tsv`, the case path
or `*` for the whole file, and a reason starting with its kind:

- `n/a:` Electron, React, IPC or TypeScript-only behaviour with no Corvene
  counterpart to test,
- `omitted:` a feature Corvene leaves out by design,
- `todo:` a `.docs/TODO.md` item that is not built yet (prefer porting and
  ignoring with `ghd: todo:` when the API exists),
- `deviation:` a documented `.docs/deviations.md` difference that makes
  the case moot; cite its flag when it has one (`deviation: NNN-slug: …`).

`check.py` reports a reason with any other start as an error.

## Updating to a new GitHub Desktop release

1. Unpack the new release next to the old one, e.g.
   `.docs/ghd-src/desktop-X.Y.Z`.
2. `python3 tools/ghd-tests/extract.py .docs/ghd-src/desktop-X.Y.Z/app/test >
   tools/ghd-tests/upstream.tsv`, then review `git diff
   tools/ghd-tests/upstream.tsv` (line numbers change too; compare
   `cut -f1,2` of both versions for the real changes).
3. `check.py` now reports markers and skips of removed or renamed cases as
   errors (move the marker, or delete the test) and new cases as missing
   (port them). Diff each changed test file between the two releases and
   update the ported module where an existing case changed.
4. `diff -r` the old and new `app/test/helpers` and
   `app/test/fixtures`; port helper changes to `corvene-test-support`, copy
   changed fixtures byte for byte into `crates/corvene-test-support/fixtures`
   (keeping its `LICENSE`, whose note names the release) and verify with
   `diff -r` (only `LICENSE` may differ).
