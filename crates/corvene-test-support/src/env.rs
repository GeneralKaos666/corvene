//! Process environment for tests that run git (the CLI or `gix`).
//!
//! GitHub Desktop's `app/test/unit-test-env.ts` fixes the commit identity
//! and blanks `HOME` / `USERPROFILE` so no test reads the developer's own
//! configuration. Corvene does the same, except that `HOME` points at an
//! empty directory instead of `''`, so tests of `git config --global` have
//! somewhere to write:
//!
//! - `HOME` and `USERPROFILE`: an empty temporary directory that lives as
//!   long as the process ([`home_dir`]; removed at exit),
//! - `XDG_CONFIG_HOME` is removed, as GitHub Desktop's CI runs without it:
//!   git and `gix` then read `$HOME/.config/git/config`, which follows a
//!   `HOME` passed to one command (GitHub Desktop's `env: { HOME }`),
//! - `GIT_CONFIG_NOSYSTEM=1`: no `/etc/gitconfig` or Homebrew/Xcode
//!   installation config,
//! - `TERM=dumb`, as GitHub Desktop's `git()` (`lib/git/core.ts`) sets for
//!   every command: a command that wants an editor fails at once instead of
//!   starting `vi` on the developer's terminal,
//! - `GIT_AUTHOR_NAME` / `GIT_COMMITTER_NAME` = [`AUTHOR_NAME`],
//!   `GIT_AUTHOR_EMAIL` / `GIT_COMMITTER_EMAIL` = [`AUTHOR_EMAIL`],
//! - every other inherited `GIT_*` variable (`GIT_DIR`, `GIT_WORK_TREE`,
//!   `GIT_INDEX_FILE`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_PARAMETERS`,
//!   `GIT_EDITOR`, `GIT_TRACE`…) and `EDITOR` / `VISUAL` are removed,
//! - the proxy variables (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` in either
//!   case) are removed and `NO_PROXY` / `no_proxy` are
//!   `127.0.0.1,localhost,::1`, so requests to the local stub servers
//!   ([`crate::http`]) never go through a proxy.
//!
//! Setting variables is `unsafe` in edition 2024 because another thread may
//! read the environment at the same time. The setup therefore runs only
//! before `main` (a `ctor` constructor), while the process has a single
//! thread, in every test binary that links this crate (a binary links it as
//! soon as it uses any item of it). [`init`] never changes the environment:
//! it fails the test if the constructor did not run. Every helper calls it,
//! and a test that runs git without any helper calls it first.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// `GIT_AUTHOR_NAME` / `GIT_COMMITTER_NAME` in every test
/// (`unit-test-env.ts`).
pub const AUTHOR_NAME: &str = "Joe Bloggs";

/// `GIT_AUTHOR_EMAIL` / `GIT_COMMITTER_EMAIL` in every test
/// (`unit-test-env.ts`).
pub const AUTHOR_EMAIL: &str = "joe.bloggs@somewhere.com";

/// The isolated home directory, or why it could not be set up.
static HOME: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Set by the constructor, so a test can prove it ran before `main`.
static RAN_BEFORE_MAIN: AtomicBool = AtomicBool::new(false);

/// Inherited variables removed besides every `GIT_*` one, compared in upper
/// case (`XDG_CONFIG_HOME` would move the global configuration out of
/// `HOME`; a proxy would see the stub servers' requests).
const REMOVED: &[&str] = &[
    "EDITOR",
    "VISUAL",
    "XDG_CONFIG_HOME",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
];

/// `NO_PROXY` / `no_proxy` in every test: the stub servers' hosts.
const NO_PROXY: &str = "127.0.0.1,localhost,::1";

#[ctor::ctor(unsafe)]
fn isolate_before_main() {
    RAN_BEFORE_MAIN.store(true, Ordering::SeqCst);
    // SAFETY: module constructors run before `main`, while the process has
    // only the thread that runs them, so nothing reads the environment
    // concurrently
    HOME.get_or_init(|| unsafe { isolate() });
}

/// Make sure the environment is isolated (see the module docs). Idempotent
/// and cheap; every helper of this crate calls it.
///
/// # Panics
///
/// When the constructor did not run before `main` (setting variables from
/// a test thread would race with the other tests), or the isolated home
/// directory could not be created.
pub fn init() {
    assert!(
        isolated_before_main(),
        "corvene-test-support: the environment isolation did not run before `main`"
    );
    match HOME.get() {
        Some(Ok(_)) => {}
        Some(Err(err)) => {
            panic!("corvene-test-support: could not isolate the test environment: {err}")
        }
        None => unreachable!("the constructor always fills HOME"),
    }
}

/// Whether the constructor isolated the environment before `main` (it
/// always should; [`init`] fails the test otherwise).
pub fn isolated_before_main() -> bool {
    RAN_BEFORE_MAIN.load(Ordering::SeqCst)
}

/// The empty directory `HOME` points at for the whole process.
pub fn home_dir() -> &'static Path {
    init();
    match HOME.get() {
        Some(Ok(home)) => home,
        _ => unreachable!("init() panics unless the home directory exists"),
    }
}

/// # Safety
///
/// No other thread may read or write the environment meanwhile.
unsafe fn isolate() -> Result<PathBuf, String> {
    let home = make_home()?;

    let removed: Vec<OsString> = std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| {
            let upper = key.to_string_lossy().to_ascii_uppercase();
            upper.starts_with("GIT_") || REMOVED.contains(&upper.as_str())
        })
        .collect();
    let set: [(&str, &std::ffi::OsStr); 10] = [
        ("HOME", home.as_os_str()),
        ("USERPROFILE", home.as_os_str()),
        ("TERM", "dumb".as_ref()),
        ("GIT_CONFIG_NOSYSTEM", "1".as_ref()),
        ("GIT_AUTHOR_NAME", AUTHOR_NAME.as_ref()),
        ("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL.as_ref()),
        ("GIT_COMMITTER_NAME", AUTHOR_NAME.as_ref()),
        ("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL.as_ref()),
        ("NO_PROXY", NO_PROXY.as_ref()),
        ("no_proxy", NO_PROXY.as_ref()),
    ];
    // SAFETY: the caller guarantees no concurrent environment access
    unsafe {
        for key in removed {
            std::env::remove_var(key);
        }
        for (key, value) in set {
            std::env::set_var(key, value);
        }
        libc::atexit(remove_home);
    }
    Ok(home)
}

/// `<temp>/corvene-test-home-<pid>-<n>`, canonical on Unix (macOS' `/var`
/// is a symlink to `/private/var`; Windows' canonical form is a `\\?\`
/// path git does not take). Plain `std` only: this runs before `main`.
fn make_home() -> Result<PathBuf, String> {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    for n in 0..1000 {
        let dir = base.join(format!("corvene-test-home-{pid}-{n}"));
        match std::fs::create_dir(&dir) {
            #[cfg(not(windows))]
            Ok(()) => {
                return std::fs::canonicalize(&dir).map_err(|e| format!("{}: {e}", dir.display()));
            }
            #[cfg(windows)]
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        }
    }
    Err(format!(
        "no free corvene-test-home-{pid}-* in {}",
        base.display()
    ))
}

extern "C" fn remove_home() {
    if let Some(Ok(home)) = HOME.get() {
        let _ = std::fs::remove_dir_all(home);
    }
}

static GLOBAL_CONFIG: Mutex<()> = Mutex::new(());

/// Exclusive use of the process-wide global git configuration
/// ([`lock_global_config`]). Dropping it empties the configuration again
/// (without removing the files; see [`lock_global_config`]).
#[must_use = "the global configuration is only reserved while the guard lives"]
pub struct GlobalConfigGuard {
    _lock: MutexGuard<'static, ()>,
}

impl GlobalConfigGuard {
    /// `$HOME/.gitconfig`, the file `git config --global` writes.
    pub fn path(&self) -> PathBuf {
        home_dir().join(".gitconfig")
    }
}

impl Drop for GlobalConfigGuard {
    fn drop(&mut self) {
        reset_global_config();
    }
}

/// Reserve the global git configuration for one test. Tests run on parallel
/// threads but share one `HOME`, so a test that writes the global
/// configuration (`git config --global`, `set_global_config_value`,
/// `add_safe_directory`…) or whose result depends on it being empty (the
/// default branch of `git init`) holds this guard for its whole body. The
/// configuration is empty when the guard is handed out and is emptied again
/// when it is dropped. Waits for the test that holds it; a panicking holder
/// does not poison it.
///
/// Other tests keep running git meanwhile, and git reads the global
/// configuration files of every command: it checks that a file exists, then
/// opens it, and dies with "unknown error occurred while reading the
/// configuration files" when the file disappears in between. So emptying
/// never removes `$HOME/.gitconfig` or `$HOME/.config/git/config`: a file
/// with content is replaced by an empty one in a single `rename` (an empty
/// file reads as no configuration), and a missing one stays missing.
pub fn lock_global_config() -> GlobalConfigGuard {
    init();
    let lock = GLOBAL_CONFIG.lock().unwrap_or_else(PoisonError::into_inner);
    reset_global_config();
    GlobalConfigGuard { _lock: lock }
}

fn reset_global_config() {
    let home = home_dir();
    empty_config_file(&home.join(".gitconfig"));
    let xdg = home.join(".config").join("git");
    let Ok(entries) = std::fs::read_dir(&xdg) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == "config" {
            empty_config_file(&path);
        } else if path.is_dir() {
            // `ignore`, `attributes`, `credentials`: git treats a missing
            // one as empty at any moment
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Replace the configuration file `path` by an empty one in one `rename`
/// (see [`lock_global_config`]); nothing to do when it is missing or
/// already empty.
fn empty_config_file(path: &Path) {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() && meta.len() > 0 => {}
        _ => return,
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".corvene-test-reset");
    let temp = PathBuf::from(temp);
    if std::fs::write(&temp, b"").is_err() || std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}
