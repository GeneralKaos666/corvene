//! Android: where Corvene keeps repositories, and the bridge to what only
//! the activity's Java side can do (pickers, permissions, intents).
//!
//! Storage. Clones live in the app-private storage, a full Linux filesystem
//! (symbolic links, file modes, no case folding): `files/repositories`, which
//! `CorveneDocumentsProvider` shows to other applications. Uninstalling the
//! application deletes it. Shared storage (`/storage/emulated/0`) is only
//! usable with "All files access" (the `foss` flavour), and then without
//! symbolic links and file modes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// What the activity does for the rest of the application; implemented over
/// JNI by the binary crate, which owns the activity.
pub trait Bridge: Send + Sync {
    /// "All files access" (`MANAGE_EXTERNAL_STORAGE`) is granted.
    fn has_all_files_access(&self) -> bool;
    /// This build may ask for it (the `foss` flavour declares it).
    fn can_request_all_files_access(&self) -> bool;
    /// This build may load native code it downloaded (the `foss` flavour;
    /// Google Play forbids it).
    fn allows_downloaded_code(&self) -> bool;
    /// Where the Play feature module with the tree-sitter grammars put its
    /// libraries, when it is installed (`play` flavour).
    fn grammar_module_dir(&self) -> Option<PathBuf>;
    /// Asks Google Play for that module; progress and the end come through
    /// [`grammar_module_event`].
    fn install_grammar_module(&self);
    /// Lets Google Play remove the module (it does so later, in the
    /// background).
    fn uninstall_grammar_module(&self);
    /// Opens the system page where the user grants it.
    fn request_all_files_access(&self);
    /// Whether notifications may be posted: `None` before the user was
    /// asked (Android 13+).
    fn notifications_allowed(&self) -> Option<bool>;
    /// Asks for `POST_NOTIFICATIONS` (Android 13+).
    fn request_notification_permission(&self);
    /// Posts a notification; a tap on it comes back through
    /// [`crate::notifications::clicked`].
    fn show_notification(&self, identifier: &str, title: &str, body: &str, payload: &str);
    /// Whether the application `package` is installed.
    fn package_installed(&self, package: &str) -> bool;
    /// Opens a file with the application the user picks for it, or a folder
    /// in the file manager (`ACTION_VIEW` on a document of
    /// `CorveneDocumentsProvider`).
    fn view_path(&self, path: &Path) -> Result<(), String>;
    /// The applications that open a text file: (label, "package/class").
    fn view_apps(&self) -> Vec<(String, String)>;
    /// The launcher icon of the application `key` names ("package" or
    /// "package/class") as PNG bytes.
    fn app_icon(&self, key: &str) -> Option<Vec<u8>>;
    /// Opens a file in the application `component` names.
    /// `line` (1-based) reaches the applications that can jump to one.
    fn view_path_with(&self, path: &Path, component: &str, line: Option<u32>)
    -> Result<(), String>;
    /// Like [`Bridge::view_path`], always offering the choice of application.
    fn view_path_with_chooser(&self, path: &Path) -> Result<(), String>;
    /// Sends a file to another application through the share sheet
    /// (`ACTION_SEND`).
    fn share_path(&self, path: &Path) -> Result<(), String>;
    /// Opens a Termux session in `dir` (Termux's `RUN_COMMAND` intent).
    fn open_termux(&self, dir: &Path) -> Result<(), String>;
    /// The root of shared storage (`Environment.getExternalStorageDirectory`,
    /// `/storage/emulated/0` for the primary user).
    fn shared_storage_dir(&self) -> Option<PathBuf>;
    /// Runs `program` (a name in Termux's `bin`) with `arguments` in a new
    /// Termux session in `dir`.
    fn run_termux(&self, program: &str, arguments: &[String], dir: &Path) -> Result<(), String>;
    /// Which of `candidates` Termux has installed (it is asked with a
    /// background command; blocks for a moment). `None` while Termux has
    /// never answered: the user has not allowed its commands yet.
    fn termux_programs(&self, candidates: &[&str]) -> Option<Vec<String>>;
    /// A network operation runs (or the last one ended): the activity keeps
    /// a foreground service while one does.
    fn transfer_active(&self, active: bool);
    /// Shows or hides the system's status and navigation bars.
    fn toggle_full_screen(&self);
    /// Starts the application again in a new process and ends this one.
    fn relaunch(&self);
    /// Shows the activity again, closing a browser tab opened over it.
    fn bring_to_front(&self);
    /// A short message over whatever is on screen (a toast).
    fn toast(&self, message: &str);
}

static NETWORK_COMMANDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A git command that talks to a remote started or ended (the observer of
/// `corvene_git::process`): keeps the process alive while one runs.
pub fn network_command(started: bool) {
    use std::sync::atomic::Ordering;
    let running = if started {
        NETWORK_COMMANDS.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        NETWORK_COMMANDS
            .fetch_sub(1, Ordering::SeqCst)
            .saturating_sub(1)
    };
    if ((started && running == 1) || (!started && running == 0))
        && let Some(bridge) = bridge()
    {
        bridge.transfer_active(started);
    }
}

/// View > Toggle full screen: shows or hides the system's bars.
pub fn toggle_full_screen() {
    if let Some(bridge) = bridge() {
        bridge.toggle_full_screen();
    }
}

/// A toast: a short message that shows over the browser too.
pub fn toast(message: &str) {
    if let Some(bridge) = bridge() {
        bridge.toast(message);
    }
}

/// Back to Corvene from the browser tab a sign-in ran in.
pub fn bring_to_front() {
    if let Some(bridge) = bridge() {
        bridge.bring_to_front();
    }
}

/// Starts Corvene again (a flag that needs a relaunch changed).
pub fn relaunch() {
    if let Some(bridge) = bridge() {
        bridge.relaunch();
    }
}

/// Whether a network command runs.
pub fn network_busy() -> bool {
    NETWORK_COMMANDS.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// Termux, the terminal most Android git users already have.
pub const TERMUX_PACKAGE: &str = "com.termux";

/// What a Termux user pastes once so the two work on the same repositories
/// (Options › Integrations copies it):
///
/// * `allow-external-apps`: lets "Open in Termux" start a session
///   (Termux's `RUN_COMMAND` intent is refused without it);
/// * `termux-setup-storage`: `~/storage/shared`, the shared storage both
///   applications reach (neither can read the other's private storage);
/// * `safe.directory`: files on shared storage belong to neither
///   application's user, which git refuses by default;
/// * `corvene [dir]`: opens a repository in Corvene from the shell, through
///   the `x-corvene://openLocalRepo` link;
/// * `corvene-git-config`: offers Termux's global git settings to Corvene
///   (`x-corvene://importGitConfig`), whose own git cannot read them.
pub const TERMUX_SETUP: &str = r#"mkdir -p ~/.termux
grep -qs '^allow-external-apps *= *true' ~/.termux/termux.properties || echo 'allow-external-apps = true' >> ~/.termux/termux.properties
termux-reload-settings
[ -d ~/storage/shared ] || termux-setup-storage
git config --global --get-all safe.directory | grep -qxF '/storage/emulated/0/*' || git config --global --add safe.directory '/storage/emulated/0/*'
grep -qs '^corvene()' ~/.bashrc || cat >> ~/.bashrc <<'CORVENE'
corvene() { am start -a android.intent.action.VIEW -d "x-corvene://openLocalRepo$(realpath "${1:-.}" | sed 's/%/%25/g; s/ /%20/g')" > /dev/null; }
CORVENE
grep -qs '^corvene-git-config()' ~/.bashrc || cat >> ~/.bashrc <<'CORVENE'
corvene-git-config() { am start -a android.intent.action.VIEW -d "x-corvene://importGitConfig/$(git config --global --list | base64 -w0 | tr '+/' '-_' | tr -d '=')" > /dev/null; }
CORVENE
. ~/.bashrc
"#;

/// Opens `path` for [`crate::apps::show_item_in_folder`] and the editor
/// integration.
pub fn view_path(path: &Path) -> std::io::Result<()> {
    let bridge = bridge().ok_or_else(|| std::io::Error::other("no activity"))?;
    bridge.view_path(path).map_err(std::io::Error::other)
}

static BACKGROUND_FETCH: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// What [`background_fetch`] runs: set once by the application.
pub fn set_background_fetch_handler(handler: impl Fn() + Send + Sync + 'static) {
    let _ = BACKGROUND_FETCH.set(Box::new(handler));
}

/// WorkManager's periodic work woke the process; false when nothing listens.
pub fn background_fetch() -> bool {
    match BACKGROUND_FETCH.get() {
        Some(handler) => {
            handler();
            true
        }
        None => false,
    }
}

/// The URL [`crate::notifications::settings_url`] gives on Android; the
/// application's URL handler opens the system's notification settings for it.
pub const NOTIFICATION_SETTINGS_URL: &str = "x-corvene-android://notification-settings";

static BRIDGE: OnceLock<Box<dyn Bridge>> = OnceLock::new();

pub fn set_bridge(bridge: Box<dyn Bridge>) {
    let _ = BRIDGE.set(bridge);
}

pub fn bridge() -> Option<&'static dyn Bridge> {
    BRIDGE.get().map(|bridge| bridge.as_ref())
}

/// `files`: the parent of `HOME` (`files/home`, set at start-up).
fn files_dir() -> PathBuf {
    dirs::home_dir()
        .and_then(|home| home.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/corvene"))
}

/// `files/repositories`: where clones go and what the documents provider
/// shows.
pub fn repositories_dir() -> PathBuf {
    files_dir().join("repositories")
}

/// Whether `path` is on shared storage, whose filesystem keeps neither
/// symbolic links nor file modes and belongs to another user id.
pub fn is_shared_storage(path: &Path) -> bool {
    path.starts_with("/storage") || path.starts_with("/sdcard") || path.starts_with("/mnt")
}

/// Why "Open in Termux" (the shell and the Termux editors) refuses a
/// repository in Corvene's own storage; the dialog that shows it offers to
/// move the repository (`Dispatcher::move_to_shared_storage`).
pub const TERMUX_PRIVATE_STORAGE: &str = "Termux cannot reach a repository in Corvene's own \
     storage. Move the repository to shared storage (a folder under /storage/emulated/0) to \
     work on it in both.";

static IMPORTED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// Remembers that `path` was copied in through the Storage Access Framework,
/// which carries no file modes.
pub fn note_imported(path: &Path) {
    if let Ok(mut imported) = IMPORTED.lock() {
        imported
            .get_or_insert_with(HashSet::new)
            .insert(path.to_path_buf());
    }
}

/// Whether `path` was imported in this session (forgotten once asked).
pub fn take_imported(path: &Path) -> bool {
    IMPORTED
        .lock()
        .ok()
        .and_then(|mut imported| imported.as_mut().map(|set| set.remove(path)))
        .unwrap_or(false)
}

/// What Google Play reports while it installs the grammar module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarModuleEvent {
    Progress { received: u64, total: u64 },
    Installed,
    Failed(String),
}

type GrammarModuleHandler = Box<dyn Fn(GrammarModuleEvent) + Send + Sync>;

static GRAMMAR_MODULE: OnceLock<GrammarModuleHandler> = OnceLock::new();

pub fn set_grammar_module_handler(handler: impl Fn(GrammarModuleEvent) + Send + Sync + 'static) {
    let _ = GRAMMAR_MODULE.set(Box::new(handler));
}

/// From the activity: the module's installation moved on.
pub fn grammar_module_event(event: GrammarModuleEvent) {
    if let Some(handler) = GRAMMAR_MODULE.get() {
        handler(event);
    }
}

// ── SSH key ─────────────────────────────────────────────────────────────────
//
// The bundled OpenSSH client reads `~/.ssh` in the app-private storage,
// where no other application can put a key. Options › Integrations creates
// one with the bundled `ssh-keygen` (or imports one from a file) and shows
// its public half to copy into the account settings of the git host. The
// code is shared with the desktop's key helper (`crate::ssh_key`).

pub use crate::ssh_key::{SshImportError, import_ssh_key, ssh_public_key};

/// Creates `~/.ssh/id_ed25519` without a passphrase (the key never leaves
/// the app-private storage, and nothing could ask for one during a fetch in
/// the background) and returns its public key. An existing key is kept.
pub fn create_ssh_key() -> Result<String, String> {
    crate::ssh_key::create_ssh_key("corvene-android", None)
}

// ── handles a finger can drag ───────────────────────────────────────────────
//
// GPUI's resizable panels are dragged with the mouse. The UI records where
// their handles are each frame; a touch that starts on (or just beside) one
// is delivered as a mouse drag by the platform layer.

/// left, top, right, bottom in logical pixels, in paint order; `true` for
/// an occluder (a dialog, a menu, a foldout), which hides the handles
/// painted before it from touches
static DRAG_HANDLES: Mutex<Vec<([f32; 4], bool)>> = Mutex::new(Vec::new());

/// How far beside a handle a finger may land.
const HANDLE_SLOP: f32 = 14.0;

/// Forgets the handles of the last frame.
pub fn clear_drag_handles() {
    if let Ok(mut handles) = DRAG_HANDLES.lock() {
        handles.clear();
    }
}

/// A handle painted at this rectangle this frame.
pub fn add_drag_handle(left: f32, top: f32, right: f32, bottom: f32) {
    if let Ok(mut handles) = DRAG_HANDLES.lock() {
        handles.push(([left, top, right, bottom], false));
    }
}

/// Something painted at this rectangle this frame takes the touches on it:
/// the handles underneath (painted earlier) are not grabbed through it.
pub fn add_drag_occluder(left: f32, top: f32, right: f32, bottom: f32) {
    if let Ok(mut handles) = DRAG_HANDLES.lock() {
        handles.push(([left, top, right, bottom], true));
    }
}

/// The point on a handle that a touch at (`x`, `y`) grabs, when it is on or
/// just beside one: the touch moved onto the handle's long axis.
pub fn drag_handle_at(x: f32, y: f32) -> Option<(f32, f32)> {
    let handles = DRAG_HANDLES.lock().ok()?;
    let covered_before = handles
        .iter()
        .rposition(|&([left, top, right, bottom], occluder)| {
            occluder && x >= left && x <= right && y >= top && y <= bottom
        })
        .map_or(0, |index| index + 1);
    handles[covered_before..]
        .iter()
        .filter(|(_, occluder)| !occluder)
        .find_map(|&([left, top, right, bottom], _)| {
            let inside = x >= left - HANDLE_SLOP
                && x <= right + HANDLE_SLOP
                && y >= top - HANDLE_SLOP
                && y <= bottom + HANDLE_SLOP;
            inside.then(|| {
                if right - left >= bottom - top {
                    // a horizontal bar: keep x, centre y
                    (x.clamp(left, right), (top + bottom) / 2.0)
                } else {
                    ((left + right) / 2.0, y.clamp(top, bottom))
                }
            })
        })
}

// ── the process environment (shared by the GPUI and the Compose app) ────────

/// An Android process starts with no `HOME` and an unwritable temporary
/// directory. Everything Corvene and git keep lives in the app-private
/// storage (`/data/user/0/<package>`), a full Linux filesystem:
///
/// * `files/home`: `HOME` (git's global config, `~/.ssh`, the XDG data and
///   state directories)
/// * `cache`: `XDG_CACHE_HOME`, which the system may clear
/// * `cache/tmp`: `TMPDIR`
/// * `files/git/bin`: the bundled git ([`bundled_git`])
pub fn prepare_environment(files: &Path) {
    let home = files.join("home");
    let cache = files
        .parent()
        .unwrap_or(Path::new("/data/local/tmp"))
        .join("cache");
    let tmp = cache.join("tmp");
    for dir in [&home, &tmp] {
        let _ = std::fs::create_dir_all(dir);
    }
    let git = bundled_git(files);
    // SAFETY: nothing else in the process reads the environment yet: this
    // runs first on the native thread, before any Rust thread is spawned
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CACHE_HOME", &cache);
        std::env::set_var("TMPDIR", &tmp);
        if let Some(git) = git {
            // `corvene_git::find_git` and git itself (ssh, git-lfs)
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut dirs = vec![git.bin.clone()];
            dirs.extend(std::env::split_paths(&path));
            if let Ok(path) = std::env::join_paths(dirs) {
                std::env::set_var("PATH", path);
            }
            std::env::set_var("CORVENE_GIT", git.bin.join("git"));
            // credentials: git runs the helper, which asks `serve_askpass`
            std::env::set_var("CORVENE_ASKPASS_PROGRAM", git.bin.join("corvene-askpass"));
            std::env::set_var("CORVENE_ASKPASS_SOCKET", cache.join("askpass.sock"));
            std::env::set_var("GIT_EXEC_PATH", &git.bin);
            std::env::set_var("GIT_TEMPLATE_DIR", &git.templates);
            // No terminal to ask whether an unknown host's key is right:
            // the first key seen is kept in ~/.ssh/known_hosts and a
            // changed one is still refused.
            if std::env::var_os("GIT_SSH_COMMAND").is_none() {
                std::env::set_var(
                    "GIT_SSH_COMMAND",
                    format!(
                        "{} -o StrictHostKeyChecking=accept-new",
                        git.bin.join("ssh").display()
                    ),
                );
            }
            // there is no /etc/gitconfig
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            // The system's trusted certificates. Android names the files by
            // OpenSSL's old subject hash, which OpenSSL 3 does not look up,
            // so the directory alone verifies nothing: the same
            // certificates are also handed over as one bundle.
            std::env::set_var("GIT_SSL_CAPATH", "/system/etc/security/cacerts");
            if let Some(bundle) = &git.ca_bundle {
                std::env::set_var("GIT_SSL_CAINFO", bundle);
            }
        }
    }
}

pub struct BundledGit {
    /// `git`, `git-remote-https`, `ssh`, `git-lfs`, …: `GIT_EXEC_PATH`.
    pub bin: PathBuf,
    /// An empty `GIT_TEMPLATE_DIR` (git warns about a missing one).
    pub templates: PathBuf,
    /// The system's certificate store as one PEM file.
    pub ca_bundle: Option<PathBuf>,
}

/// The executables the package carries as `lib*.so` (the only files an app
/// may execute are those the installer extracts into its native library
/// directory; `packaging/android/git/build.sh`), under the names git looks
/// for: symbolic links in `files/git/bin`, made again on every start because
/// the library directory moves with each update.
pub fn bundled_git(files: &Path) -> Option<BundledGit> {
    const LINKS: &[(&str, &str)] = &[
        ("git", "libgit.so"),
        ("git-remote-https", "libgit-remote-https.so"),
        ("git-remote-http", "libgit-remote-https.so"),
        ("ssh", "libssh.so"),
        ("ssh-keygen", "libssh-keygen.so"),
        ("git-lfs", "libgit-lfs.so"),
        ("corvene-askpass", "libcorvene-askpass.so"),
        ("git-sh-setup", "libgit-sh-setup.so"),
        ("git-sh-i18n", "libgit-sh-i18n.so"),
        ("git-submodule", "libgit-submodule.so"),
        ("git-mergetool", "libgit-mergetool.so"),
        ("git-mergetool--lib", "libgit-mergetool--lib.so"),
    ];
    let libraries = native_library_dir()?;
    if !libraries.join("libgit.so").exists() {
        return None;
    }
    let bin = files.join("git/bin");
    let templates = files.join("git/templates");
    let _ = std::fs::remove_dir_all(&bin);
    for dir in [&bin, &templates] {
        std::fs::create_dir_all(dir).ok()?;
    }
    for (name, library) in LINKS {
        let target = libraries.join(library);
        if target.exists() {
            let _ = std::os::unix::fs::symlink(target, bin.join(name));
        }
    }
    let ca_bundle = ca_bundle(&files.join("git/cacert.pem"));
    Some(BundledGit {
        bin,
        templates,
        ca_bundle,
    })
}

/// Writes the certificates Android trusts (the Conscrypt module's store,
/// which replaced `/system/etc/security/cacerts` in Android 14, and the ones
/// the user installed) into `bundle`, on every start so removals and
/// additions are followed.
fn ca_bundle(bundle: &Path) -> Option<PathBuf> {
    let system = [
        "/apex/com.android.conscrypt/cacerts",
        "/system/etc/security/cacerts",
    ]
    .into_iter()
    .find(|dir| Path::new(dir).is_dir())?;
    let mut pem = Vec::new();
    for dir in [system, "/data/misc/user/0/cacerts-added"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(certificate) = std::fs::read(entry.path()) {
                pem.extend_from_slice(&certificate);
                pem.push(b'\n');
            }
        }
    }
    if pem.is_empty() {
        return None;
    }
    std::fs::write(bundle, pem).ok()?;
    Some(bundle.to_path_buf())
}

/// Where the installer extracted the package's native libraries: the
/// directory this library was loaded from.
pub fn native_library_dir() -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: `dladdr` fills `info` for an address inside this library and
    // `dli_fname` then points at the loader's own NUL-terminated path
    let path = unsafe {
        if libc::dladdr(native_library_dir as *const libc::c_void, info.as_mut_ptr()) == 0 {
            return None;
        }
        let name = info.assume_init().dli_fname;
        if name.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(name)
            .to_string_lossy()
            .into_owned()
    };
    Path::new(&path).parent().map(Path::to_path_buf)
}
