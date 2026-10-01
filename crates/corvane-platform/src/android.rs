//! Android: where Corvane keeps repositories, and the bridge to what only
//! the activity's Java side can do (pickers, permissions, intents).
//!
//! Storage. Clones live in the app-private storage, a full Linux filesystem
//! (symbolic links, file modes, no case folding): `files/repositories`, which
//! `CorvaneDocumentsProvider` shows to other applications. Uninstalling the
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
    /// `CorvaneDocumentsProvider`).
    fn view_path(&self, path: &Path) -> Result<(), String>;
    /// The applications that open a text file: (label, "package/class").
    fn view_apps(&self) -> Vec<(String, String)>;
    /// Opens a file in the application `component` names.
    fn view_path_with(&self, path: &Path, component: &str) -> Result<(), String>;
    /// Like [`Bridge::view_path`], always offering the choice of application.
    fn view_path_with_chooser(&self, path: &Path) -> Result<(), String>;
    /// Sends a file to another application through the share sheet
    /// (`ACTION_SEND`).
    fn share_path(&self, path: &Path) -> Result<(), String>;
    /// Opens a Termux session in `dir` (Termux's `RUN_COMMAND` intent).
    fn open_termux(&self, dir: &Path) -> Result<(), String>;
    /// A network operation runs (or the last one ended): the activity keeps
    /// a foreground service while one does.
    fn transfer_active(&self, active: bool);
}

static NETWORK_COMMANDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A git command that talks to a remote started or ended (the observer of
/// `corvane_git::process`): keeps the process alive while one runs.
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

/// Whether a network command runs.
pub fn network_busy() -> bool {
    NETWORK_COMMANDS.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// Termux, the terminal most Android git users already have.
pub const TERMUX_PACKAGE: &str = "com.termux";

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
pub const NOTIFICATION_SETTINGS_URL: &str = "x-corvane-android://notification-settings";

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
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/corvane"))
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
// one with the bundled `ssh-keygen` and shows its public half to copy into
// the account settings of the git host.

fn ssh_key_file() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".ssh").join("id_ed25519"))
}

static SSH_PUBLIC_KEY: Mutex<Option<Option<String>>> = Mutex::new(None);

/// The public key of `~/.ssh/id_ed25519` (read once, then remembered).
pub fn ssh_public_key() -> Option<String> {
    let mut cached = SSH_PUBLIC_KEY.lock().ok()?;
    cached
        .get_or_insert_with(|| {
            let public = ssh_key_file()?.with_extension("pub");
            let key = std::fs::read_to_string(public).ok()?;
            Some(key.trim().to_string()).filter(|key| !key.is_empty())
        })
        .clone()
}

/// Creates `~/.ssh/id_ed25519` without a passphrase (the key never leaves
/// the app-private storage, and nothing could ask for one during a fetch in
/// the background) and returns its public key. An existing key is kept.
pub fn create_ssh_key() -> Result<String, String> {
    if let Some(key) = ssh_public_key() {
        return Ok(key);
    }
    let file = ssh_key_file().ok_or("no home directory")?;
    let dir = file.parent().ok_or("no home directory")?;
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let keygen = std::env::var_os("GIT_EXEC_PATH")
        .map(|bin| PathBuf::from(bin).join("ssh-keygen"))
        .filter(|keygen| keygen.exists())
        .ok_or("this build has no ssh-keygen")?;
    let output = std::process::Command::new(keygen)
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            "corvane-android",
            "-f",
        ])
        .arg(&file)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    if let Ok(mut cached) = SSH_PUBLIC_KEY.lock() {
        *cached = None;
    }
    ssh_public_key().ok_or_else(|| "ssh-keygen wrote no public key".to_string())
}
