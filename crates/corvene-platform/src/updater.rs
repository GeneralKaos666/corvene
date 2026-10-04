//! Self-updater - the place of Electron's `autoUpdater` + Squirrel in GHD
//! (`main-process/main.ts` `autoUpdater.*`, `ui/lib/update-store.ts`),
//! where the GitHub Releases API is the feed
//! (`GET /repos/wasi-master/corvene/releases/latest`).
//!
//! macOS: the release's `.zip` is downloaded to
//! `~/Library/Caches/Corvene/updates/`, checked against the sha256 the feed
//! lists for it (the asset's `digest`, which GitHub computes at upload),
//! unpacked with `ditto`
//! (keeps the code signature intact), swapped in for the running bundle
//! (`Corvene.app` → `Corvene.app.old`, new bundle moved in) and opened again
//! once this process has exited. The `.old` bundle is removed at the next
//! launch. Files this app writes carry no quarantine attribute, so the
//! self-signed update launches without a Gatekeeper prompt. A bundle
//! installed by Homebrew is never swapped: `brew upgrade corvene` owns it
//! (see [`is_homebrew_install`]).
//!
//! Linux (GHD ships no Linux build; Squirrel has no Linux backend): only an
//! AppImage updates itself. `$APPIMAGE` names the running image
//! ([`running_appimage`]); the release's `Corvene-<v>-<arch>.AppImage` is
//! downloaded to `$XDG_CACHE_HOME/corvene/updates/` and checked like the zip, then
//! [`install`] copies the image next to `$APPIMAGE` under a temporary name,
//! verifies that copy again, makes it executable, fsyncs it and renames it
//! over `$APPIMAGE` (atomic: a crash leaves the old or the new image, never
//! half of one). The relaunch runs the new image once this process has
//! exited (`app_location::relaunch_after_exit`). Any other install (the
//! `.deb` in `/usr/lib/corvene`, a bare binary) belongs to the package
//! manager, like a Homebrew cask ([`package_manager`]). So does an AppImage
//! the Homebrew cask installed ([`is_homebrew_appimage`]): `brew upgrade
//! corvene` replaces it.
//!
//! Testing hook: `CORVENE_UPDATE_FEED=<url>` replaces the feed URL.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tracing::{debug, info, warn};

/// GHD `__UPDATES_URL__`: the release feed.
pub const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/wasi-master/corvene/releases/latest";

const USER_AGENT: &str = concat!("Corvene/", env!("CARGO_PKG_VERSION"));

/// What the updater installs on this OS, for error messages.
#[cfg(target_os = "macos")]
const ASSET_KIND: &str = "macOS .zip";
#[cfg(windows)]
const ASSET_KIND: &str = "Windows installer";
#[cfg(not(any(target_os = "macos", windows)))]
const ASSET_KIND: &str = "AppImage";
#[cfg(target_os = "macos")]
const ASSET_EXTENSION: &str = ".zip";
#[cfg(windows)]
const ASSET_EXTENSION: &str = "-setup.exe";
#[cfg(not(any(target_os = "macos", windows)))]
const ASSET_EXTENSION: &str = ".AppImage";

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("the release feed could not be reached: {0}")]
    Network(String),
    #[error("the release feed answered with status {0}")]
    Status(u16),
    #[error("the release feed could not be read: {0}")]
    Feed(String),
    #[error("release {0} has no {ASSET_KIND} asset")]
    NoAsset(String),
    #[error("release {0} lists no sha256 digest for its {ASSET_EXTENSION}")]
    NoDigest(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("the download does not match the release's sha256 ({0})")]
    Checksum(String),
    #[error("{0}")]
    Install(String),
}

/// A newer release than the running version, with the asset to install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// `tag_name` without its `v`.
    pub version: String,
    pub tag: String,
    pub name: Option<String>,
    /// The Markdown release notes.
    pub body: Option<String>,
    /// ISO-8601.
    pub published_at: Option<String>,
    pub html_url: String,
    /// The asset to install: the `.zip` on macOS, the AppImage on Linux
    /// (the field names predate Linux).
    pub zip_name: String,
    pub zip_url: String,
    pub zip_size: u64,
    /// Hex sha256 of the asset, from the feed's `digest`.
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Debug, Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    /// `sha256:<hex>`
    #[serde(default)]
    digest: Option<String>,
}

/// The feed URL: `CORVENE_UPDATE_FEED` or the GitHub Releases API.
pub fn feed_url() -> String {
    std::env::var("CORVENE_UPDATE_FEED")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| RELEASES_LATEST_URL.to_string())
}

fn agent(global_timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        // the system proxy too (`corvene_platform::proxy`)
        .proxy(crate::proxy::agent_proxy())
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(global_timeout)
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .new_agent()
}

/// Ask the feed for the latest release; `Ok(None)` when it is not newer
/// than `current_version` (or is a draft). `full`: this is a `Corvene-Full`
/// build, which updates to the release's `Corvene-Full-…` asset.
pub fn check_latest(current_version: &str, full: bool) -> Result<Option<ReleaseInfo>, UpdateError> {
    let url = feed_url();
    debug!(%url, "checking for updates");
    let mut response = agent(Some(Duration::from_secs(30)))
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(UpdateError::Status(status));
    }
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|err| UpdateError::Feed(err.to_string()))?;
    let release: ApiRelease =
        serde_json::from_str(&body).map_err(|err| UpdateError::Feed(err.to_string()))?;
    release_info_for(
        release,
        current_version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        full,
    )
}

/// The release's update for an `os` / `arch` pair (`std::env::consts`
/// values) and variant.
fn release_info_for(
    release: ApiRelease,
    current_version: &str,
    os: &str,
    arch: &str,
    full: bool,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    if release.draft {
        return Ok(None);
    }
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !version_is_newer(&version, current_version) {
        debug!(latest = %version, running = %current_version, "no update available");
        return Ok(None);
    }
    let Some(zip) = pick_asset(&release.assets, os, arch, full) else {
        return Err(UpdateError::NoAsset(release.tag_name));
    };
    let Some(sha256) = zip.digest.as_deref().and_then(sha256_digest) else {
        return Err(UpdateError::NoDigest(release.tag_name));
    };
    Ok(Some(ReleaseInfo {
        version,
        tag: release.tag_name,
        name: release.name,
        body: release.body,
        published_at: release.published_at,
        html_url: release.html_url,
        zip_name: zip.name.clone(),
        zip_url: zip.browser_download_url.clone(),
        zip_size: zip.size,
        sha256,
    }))
}

/// The hex of a `sha256:<hex>` asset digest.
fn sha256_digest(digest: &str) -> Option<String> {
    let hex = digest.trim().strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// The asset an `os` / `arch` machine installs: the macOS `.zip`
/// ([`pick_zip_asset`]), the Windows installer ([`pick_setup_asset`]) or,
/// anywhere else, the AppImage ([`pick_appimage_asset`]; the `.deb`, `.rpm`,
/// `.tar.gz`, Flatpak and snap are never picked). `full` picks among the
/// `Corvene-Full-…` assets, otherwise those are skipped.
fn pick_asset<'a>(
    assets: &'a [ApiAsset],
    os: &str,
    arch: &str,
    full: bool,
) -> Option<&'a ApiAsset> {
    let assets = assets.iter().filter(|a| is_variant(&a.name, full));
    if os == "macos" {
        pick_zip_asset(assets, arch)
    } else if os == "windows" {
        pick_setup_asset(assets, arch)
    } else {
        pick_appimage_asset(assets, arch)
    }
}

/// Is `name` an asset of the `Corvene-Full` variant (`full`) or of the
/// default one?
fn is_variant(name: &str, full: bool) -> bool {
    if full {
        name.starts_with("Corvene-Full-")
    } else {
        name.starts_with("Corvene-") && !name.starts_with("Corvene-Full-")
    }
}

/// The macOS `.zip` for this machine (`Corvene-<version>-macos-<arch>.zip`):
/// the universal one, else one naming this architecture. Pack archives and
/// the Windows portable zips are skipped.
fn pick_zip_asset<'a>(
    assets: impl Iterator<Item = &'a ApiAsset>,
    arch: &str,
) -> Option<&'a ApiAsset> {
    let zips: Vec<&ApiAsset> = assets
        .filter(|a| a.name.ends_with(".zip") && a.name.contains("-macos-"))
        .collect();
    let arch = match arch {
        "aarch64" => "arm64",
        other => other,
    };
    zips.iter()
        .find(|a| a.name.contains("-universal"))
        .or_else(|| zips.iter().find(|a| a.name.contains(arch)))
        .or_else(|| zips.first())
        .copied()
}

/// `Corvene[-Full]-<version>-<arch>.AppImage` (`packaging/linux/package.sh`),
/// `<arch>` being what AppImages call `std::env::consts::ARCH` (`x86_64`
/// and `aarch64` as they are, `i686` for `x86`, `armhf` for `arm`).
fn pick_appimage_asset<'a>(
    mut assets: impl Iterator<Item = &'a ApiAsset>,
    arch: &str,
) -> Option<&'a ApiAsset> {
    let arch = match arch {
        "x86" => "i686",
        "arm" => "armhf",
        other => other,
    };
    let suffix = format!("-{arch}.AppImage");
    assets.find(|a| a.name.ends_with(&suffix))
}

/// `Corvene[-Full]-<version>-<arch>-setup.exe`
/// (`packaging/windows/package.ps1`), `<arch>` being `x86_64` and `aarch64`
/// as `std::env::consts::ARCH` spells them and `i686` for `x86`; the `.msi`
/// and the portable `.zip` are never picked.
fn pick_setup_asset<'a>(
    mut assets: impl Iterator<Item = &'a ApiAsset>,
    arch: &str,
) -> Option<&'a ApiAsset> {
    let arch = match arch {
        "x86" => "i686",
        other => other,
    };
    let suffix = format!("-{arch}-setup.exe");
    assets.find(|a| a.name.ends_with(&suffix))
}

/// `MAJOR.MINOR.PATCH[-pre]`; a pre-release sorts before its release.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    parts: [u64; 3],
    /// `None` > `Some(_)` in the ordering below, so `release` is a flag first.
    release: bool,
    pre: String,
}

fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches('v');
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, pre.to_string()),
        None => (text, String::new()),
    };
    let core = core.split_once('+').map(|(c, _)| c).unwrap_or(core);
    let mut parts = [0u64; 3];
    for (ix, piece) in core.split('.').enumerate() {
        if ix >= 3 {
            return None;
        }
        parts[ix] = piece.parse().ok()?;
    }
    Some(Version {
        parts,
        release: pre.is_empty(),
        pre,
    })
}

/// Is `candidate` a newer version than `current`? Unparsable versions never are.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(r)) => c > r,
        _ => false,
    }
}

/// The Homebrew cask owns bundles it installed: under a Caskroom, or the
/// `/Applications/Corvene.app` a `corvene` cask has put there.
pub fn is_homebrew_install(bundle: &Path) -> bool {
    const CASKROOMS: [&str; 2] = ["/opt/homebrew/Caskroom", "/usr/local/Caskroom"];
    if CASKROOMS.iter().any(|room| bundle.starts_with(room)) {
        return true;
    }
    let in_applications = bundle.starts_with("/Applications");
    in_applications
        && CASKROOMS
            .iter()
            .any(|room| Path::new(room).join("corvene").is_dir())
}

/// The file or bundle an update replaces: the running `.app` on macOS
/// (`None` for a bare binary), the AppImage on Linux (`None` for a `.deb` or
/// a bare binary).
pub fn install_target() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle()
    }
    #[cfg(windows)]
    {
        installed_executable()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        running_appimage()
    }
}

/// The Homebrew cask owns the AppImage it installed on Linux: an image in
/// `~/Applications` (the cask's `app_image` target, Homebrew's
/// `appimagedir`) while one of `caskrooms` holds a `corvene` cask.
pub fn is_homebrew_appimage(image: &Path, home: &Path, caskrooms: &[PathBuf]) -> bool {
    image.starts_with(home.join("Applications"))
        && caskrooms.iter().any(|room| room.join("corvene").is_dir())
}

/// Where Homebrew on Linux keeps its casks: under `$HOMEBREW_PREFIX` (unset
/// when the desktop starts Corvene), the default prefix and the per-user one.
#[cfg(not(any(target_os = "macos", windows)))]
fn linuxbrew_caskrooms(home: &Path) -> Vec<PathBuf> {
    std::env::var_os("HOMEBREW_PREFIX")
        .filter(|prefix| !prefix.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([
            PathBuf::from("/home/linuxbrew/.linuxbrew"),
            home.join(".linuxbrew"),
        ])
        .map(|prefix| prefix.join("Caskroom"))
        .collect()
}

/// Who updates a package-managed install ([`package_manager`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageManager {
    /// The Homebrew cask: `brew upgrade corvene`.
    Homebrew,
    /// The distribution's package manager (the `.deb`), or whoever built
    /// the binary.
    System,
}

/// The package manager that owns this install, so the updater only
/// announces a release: Homebrew for a cask bundle on macOS
/// ([`is_homebrew_install`]) and a cask AppImage on Linux
/// ([`is_homebrew_appimage`]); on Linux the system's for anything that is
/// not an AppImage (the `.deb` under `/usr/lib/corvene`, a binary built from
/// source). `None`: Corvene updates itself.
pub fn package_manager() -> Option<PackageManager> {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle()
            .is_some_and(|b| is_homebrew_install(&b))
            .then_some(PackageManager::Homebrew)
    }
    #[cfg(windows)]
    {
        installed_executable()
            .is_none()
            .then_some(PackageManager::System)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let image = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .and_then(|path| std::fs::canonicalize(path).ok());
        if let (Some(image), Some(home)) = (&image, dirs::home_dir())
            && is_homebrew_appimage(image, &home, &linuxbrew_caskrooms(&home))
        {
            return Some(PackageManager::Homebrew);
        }
        running_appimage()
            .is_none()
            .then_some(PackageManager::System)
    }
}

/// Windows: this executable when the installer put it there (its
/// uninstaller sits next to it). A build run from elsewhere (`cargo run`, an
/// unpacked copy) is not the updater's to replace.
#[cfg(windows)]
pub fn installed_executable() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent()?.join("unins000.exe").is_file().then_some(exe)
}

/// The verified installer [`install`] left for the relaunch to run.
#[cfg(windows)]
static PENDING_SETUP: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Windows: the installer to run once this process has exited (see
/// [`install`]), handed out once.
#[cfg(windows)]
pub fn take_pending_setup() -> Option<PathBuf> {
    PENDING_SETUP.lock().ok()?.take()
}

/// The AppImage this process runs from: `$APPIMAGE` (set by the AppImage
/// runtime) when it names a regular file this user may replace.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn running_appimage() -> Option<PathBuf> {
    appimage_target(std::env::var_os("APPIMAGE"))
}

/// [`running_appimage`] for a given `$APPIMAGE` value, symlinks resolved so
/// the rename replaces the image itself. "May replace" means the file is
/// writable and so is its folder (the rename needs the latter); a running
/// image can answer `ETXTBSY` for the former, which counts as writable
/// since the image is replaced, never written to.
#[cfg(not(any(target_os = "macos", windows)))]
fn appimage_target(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value.filter(|v| !v.is_empty())?);
    if !path.is_absolute() {
        return None;
    }
    let path = std::fs::canonicalize(path).ok()?;
    if !std::fs::metadata(&path).ok()?.is_file() {
        return None;
    }
    let dir = path.parent()?;
    let file_ok = match writable(&path) {
        Ok(()) => true,
        Err(err) => err.raw_os_error() == Some(libc::ETXTBSY),
    };
    (file_ok && writable(dir).is_ok()).then_some(path)
}

/// `access(path, W_OK)`
#[cfg(not(any(target_os = "macos", windows)))]
fn writable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call.
    if unsafe { libc::access(c_path.as_ptr(), libc::W_OK) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// `~/Library/Caches/Corvene/updates` (Linux: `$XDG_CACHE_HOME/corvene/updates`)
pub fn updates_dir() -> PathBuf {
    crate::paths::cache_dir().join("updates")
}

/// Download the release's asset (`.zip` / AppImage) into a fresh updates
/// directory; `progress(received, total)` is called as bytes arrive. Returns
/// the asset's path.
pub fn download(
    release: &ReleaseInfo,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<PathBuf, UpdateError> {
    let dir = updates_dir();
    // older downloads and extracted bundles are never reused
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let zip = dir.join(&release.zip_name);

    let agent = agent(None);
    info!(url = %release.zip_url, "downloading update");
    let mut response = agent
        .get(&release.zip_url)
        .call()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(UpdateError::Status(status));
    }
    let total = response
        .body_mut()
        .content_length()
        .or(Some(release.zip_size).filter(|s| *s > 0));
    let part = dir.join(format!("{}.part", release.zip_name));
    let mut file = std::fs::File::create(&part)?;
    let mut reader = response.body_mut().as_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    progress(0, total);
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        received += n as u64;
        progress(received, total);
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&part, &zip)?;
    Ok(zip)
}

/// Check that `file` hashes to `sha256` ([`ReleaseInfo::sha256`]).
pub fn verify(file: &Path, sha256: &str) -> Result<(), UpdateError> {
    let actual = crate::file_hash::get_file_hash(file, crate::file_hash::HashAlgorithm::Sha256)?;
    if actual.eq_ignore_ascii_case(sha256.trim()) {
        Ok(())
    } else {
        Err(UpdateError::Checksum(actual))
    }
}

/// `<bundle>.old` next to the bundle (`Corvene.app.old`).
pub fn old_bundle_path(bundle: &Path) -> PathBuf {
    let mut name = bundle
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".old");
    bundle.with_file_name(name)
}

/// Install the verified download (`sha256`: what [`verify`] checked it
/// against) over [`install_target`]; the caller
/// relaunches (`app_location::relaunch_after_exit`) and quits. macOS: unpack
/// the zip and swap bundles ([`install_bundle`]). Linux: rename the AppImage
/// over the running one ([`install_appimage`]).
pub fn install(file: &Path, target: &Path, sha256: &str) -> Result<(), UpdateError> {
    #[cfg(target_os = "macos")]
    {
        let _ = sha256;
        install_bundle(file, target)
    }
    // Windows cannot replace a running program: the installer runs once
    // Corvene has exited, started by `app_location::relaunch_after_exit`.
    // It is copied out of the cache and the copy is what gets verified, so
    // what runs is what was verified.
    #[cfg(windows)]
    {
        let _ = target;
        let setup = updates_dir().join("pending-setup.exe");
        std::fs::copy(file, &setup)?;
        if let Err(err) = verify(&setup, sha256) {
            let _ = std::fs::remove_file(&setup);
            return Err(err);
        }
        if let Ok(mut pending) = PENDING_SETUP.lock() {
            *pending = Some(setup);
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        install_appimage(file, target, sha256)
    }
}

/// Copy `new_image` next to `running` under a temporary name, check the
/// copy against `sha256` (what gets renamed is what
/// was verified, whatever happened to the cache since the download), make it
/// `0755`, fsync it and rename it over `running`, then fsync the folder. On
/// any failure the temporary file is removed and `running` is untouched.
#[cfg(not(any(target_os = "macos", windows)))]
fn install_appimage(new_image: &Path, running: &Path, sha256: &str) -> Result<(), UpdateError> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let dir = running
        .parent()
        .ok_or_else(|| UpdateError::Install(format!("{} has no folder", running.display())))?;
    let name = running
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Corvene.AppImage".into());
    let temp = dir.join(format!(".{name}.update-{}", std::process::id()));
    let _ = std::fs::remove_file(&temp);
    let result = (|| -> Result<(), UpdateError> {
        let mut source = std::fs::File::open(new_image)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&temp)?;
        std::io::copy(&mut source, &mut file)?;
        // `mode` above is subject to the umask
        file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
        file.sync_all()?;
        drop(file);
        verify(&temp, sha256)?;
        std::fs::rename(&temp, running).map_err(|err| {
            UpdateError::Install(format!("could not replace {}: {err}", running.display()))
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    // the rename is durable once the folder is
    if let Ok(dir) = std::fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    info!(image = %running.display(), "update installed");
    Ok(())
}

/// Unpack the verified `zip` and swap it in for `running_bundle`. The old
/// bundle is left as `<bundle>.old` for [`remove_old_bundle`].
#[cfg(target_os = "macos")]
fn install_bundle(zip: &Path, running_bundle: &Path) -> Result<(), UpdateError> {
    let extracted = updates_dir().join("extracted");
    let _ = std::fs::remove_dir_all(&extracted);
    std::fs::create_dir_all(&extracted)?;
    let status = std::process::Command::new("/usr/bin/ditto")
        .args(["-x", "-k", "--sequesterRsrc"])
        .arg(zip)
        .arg(&extracted)
        .status()
        .map_err(|err| UpdateError::Install(format!("could not run ditto: {err}")))?;
    if !status.success() {
        return Err(UpdateError::Install(format!(
            "unpacking {} failed ({status})",
            zip.display()
        )));
    }
    let new_bundle = find_app_bundle(&extracted).ok_or_else(|| {
        UpdateError::Install(format!("{} contains no .app bundle", zip.display()))
    })?;
    if !new_bundle.join("Contents/MacOS/corvene").is_file() {
        return Err(UpdateError::Install(format!(
            "{} is not a Corvene bundle",
            new_bundle.display()
        )));
    }
    swap_bundles(&new_bundle, running_bundle)
}

/// The first `.app` at the top of `dir` or one level down (zips made with
/// `ditto --keepParent` wrap the bundle in a folder).
#[cfg(target_os = "macos")]
fn find_app_bundle(dir: &Path) -> Option<PathBuf> {
    let is_app = |p: &Path| p.extension().is_some_and(|e| e == "app") && p.is_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    if let Some(app) = entries.iter().find(|p| is_app(p)) {
        return Some(app.clone());
    }
    for entry in entries.iter().filter(|p| p.is_dir()) {
        let mut inner: Vec<PathBuf> = std::fs::read_dir(entry)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        inner.sort();
        if let Some(app) = inner.into_iter().find(|p| is_app(p)) {
            return Some(app);
        }
    }
    None
}

/// `running` → `running.old`, `new_bundle` → `running`; the first move is
/// undone when the second fails.
#[cfg(target_os = "macos")]
fn swap_bundles(new_bundle: &Path, running: &Path) -> Result<(), UpdateError> {
    let old = old_bundle_path(running);
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    std::fs::rename(running, &old).map_err(|err| {
        UpdateError::Install(format!("could not move {} aside: {err}", running.display()))
    })?;
    if let Err(err) = move_dir(new_bundle, running) {
        let restored = std::fs::rename(&old, running);
        return Err(UpdateError::Install(format!(
            "could not move the new bundle into {}: {err}{}",
            running.display(),
            if restored.is_ok() {
                ""
            } else {
                " (and the old bundle could not be restored)"
            }
        )));
    }
    info!(bundle = %running.display(), "update installed");
    Ok(())
}

/// Rename, or `ditto` + remove when the source is on another volume.
#[cfg(target_os = "macos")]
fn move_dir(from: &Path, to: &Path) -> Result<(), String> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(18) => {
            let status = std::process::Command::new("/usr/bin/ditto")
                .arg(from)
                .arg(to)
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("copying {} failed", from.display()));
            }
            let _ = std::fs::remove_dir_all(from);
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Delete the `<bundle>.old` a previous update left behind. `Ok(true)` when
/// there was one.
pub fn remove_old_bundle(bundle: &Path) -> std::io::Result<bool> {
    let old = old_bundle_path(bundle);
    if !old.exists() {
        return Ok(false);
    }
    match std::fs::remove_dir_all(&old) {
        Ok(()) => {
            info!(path = %old.display(), "removed the previous bundle");
            Ok(true)
        }
        Err(err) => {
            warn!(path = %old.display(), %err, "could not remove the previous bundle");
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        assert!(version_is_newer("0.1.1", "0.1.0"));
        assert!(version_is_newer("v0.2.0", "0.1.9"));
        assert!(version_is_newer("1.0.0", "0.99.99"));
        assert!(!version_is_newer("0.1.0", "0.1.0"));
        assert!(!version_is_newer("0.0.9", "0.1.0"));
        // pre-releases sort before their release
        assert!(version_is_newer("0.2.0", "0.2.0-beta.1"));
        assert!(!version_is_newer("0.2.0-beta.1", "0.2.0"));
        assert!(version_is_newer("0.2.0-beta.2", "0.2.0-beta.1"));
        assert!(version_is_newer("0.2.0-rc.1+build", "0.2.0-beta.9"));
        assert!(!version_is_newer("garbage", "0.1.0"));
        assert!(!version_is_newer("0.1.0.1", "0.1.0"));
    }

    /// sha256 of "corvene\n"
    const SHA: &str = "d685a1bffc4dd7a8a6e5699d2c994b5628f7fdc9425f56254f9b0a12c8ed14aa";

    fn asset(name: &str) -> ApiAsset {
        ApiAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 1,
            digest: Some(format!("sha256:{SHA}")),
        }
    }

    #[test]
    fn picks_the_universal_zip_and_its_digest() {
        let release = ApiRelease {
            tag_name: "v0.2.0".into(),
            name: None,
            body: Some("- [New] Things".into()),
            published_at: None,
            html_url: "https://example.invalid/r".into(),
            draft: false,
            assets: vec![
                asset("tree-sitter-packs.zip"),
                asset("Corvene-Full-0.2.0-macos-universal.zip"),
                asset("Corvene-0.2.0-macos-universal.zip"),
                asset("Corvene-0.2.0-macos-universal.dmg"),
            ],
        };
        let info = release_info_for(release, "0.1.0", "macos", "aarch64", false)
            .unwrap()
            .unwrap();
        assert_eq!(info.version, "0.2.0");
        assert_eq!(info.zip_name, "Corvene-0.2.0-macos-universal.zip");
        assert_eq!(info.sha256, SHA);
    }

    #[test]
    fn drafts_and_older_releases_are_ignored_and_missing_assets_are_errors() {
        let mut release = ApiRelease {
            tag_name: "v0.0.1".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: vec![asset("Corvene-0.0.1-macos-universal.zip")],
        };
        assert_eq!(
            release_info_for(release, "0.1.0", "macos", "x86_64", false).unwrap(),
            None
        );
        release = ApiRelease {
            tag_name: "v9.0.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: true,
            assets: vec![],
        };
        assert_eq!(
            release_info_for(release, "0.1.0", "macos", "x86_64", false).unwrap(),
            None
        );
        release = ApiRelease {
            tag_name: "v9.0.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: vec![ApiAsset {
                digest: Some("md5:00".into()),
                ..asset("Corvene-9.0.0-macos-universal.zip")
            }],
        };
        assert!(matches!(
            release_info_for(release, "0.1.0", "macos", "x86_64", false),
            Err(UpdateError::NoDigest(_))
        ));
    }

    #[test]
    fn old_bundle_sits_next_to_the_bundle() {
        assert_eq!(
            old_bundle_path(Path::new("/Applications/Corvene.app")),
            PathBuf::from("/Applications/Corvene.app.old")
        );
    }

    #[test]
    fn homebrew_detection() {
        assert!(is_homebrew_install(Path::new(
            "/opt/homebrew/Caskroom/corvene/0.1.0/Corvene.app"
        )));
        assert!(!is_homebrew_install(Path::new(
            "/Users/someone/Downloads/Corvene.app"
        )));
    }

    #[test]
    fn homebrew_appimage_detection() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let image = home.join("Applications/Corvene.AppImage");
        let caskrooms = [tmp.path().join("none"), tmp.path().join("brew/Caskroom")];
        // no `corvene` cask: a hand-downloaded image updates itself
        assert!(!is_homebrew_appimage(&image, &home, &caskrooms));
        std::fs::create_dir_all(caskrooms[1].join("corvene")).unwrap();
        assert!(is_homebrew_appimage(&image, &home, &caskrooms));
        // the cask does not own an image kept somewhere else
        assert!(!is_homebrew_appimage(
            &home.join("Downloads/Corvene-0.1.0-x86_64.AppImage"),
            &home,
            &caskrooms
        ));
    }

    #[test]
    fn verifies_a_file_against_its_sha256_and_rejects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("corvene.txt");
        std::fs::write(&file, b"corvene\n").unwrap();
        verify(&file, SHA).unwrap();
        verify(&file, &SHA.to_ascii_uppercase()).unwrap();
        std::fs::write(&file, b"corvene!\n").unwrap();
        assert!(matches!(verify(&file, SHA), Err(UpdateError::Checksum(_))));
    }

    #[test]
    fn digests_must_be_sha256() {
        assert_eq!(
            sha256_digest(&format!("sha256:{SHA}")).as_deref(),
            Some(SHA)
        );
        assert_eq!(sha256_digest("sha256:abc"), None);
        assert_eq!(sha256_digest(SHA), None);
    }

    fn release_with(assets: &[&str]) -> ApiRelease {
        ApiRelease {
            tag_name: "v0.2.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: assets.iter().map(|name| asset(name)).collect(),
        }
    }

    const LINUX_ASSETS: [&str; 7] = [
        "Corvene-0.2.0-macos-universal.zip",
        "corvene_0.2.0_amd64.deb",
        "corvene_0.2.0_arm64.deb",
        "Corvene-0.2.0-x86_64.AppImage",
        "Corvene-0.2.0-aarch64.AppImage",
        "Corvene-0.2.0-i686.AppImage",
        "Corvene-0.2.0-armhf.AppImage",
    ];

    #[test]
    fn linux_picks_the_appimage_for_its_architecture() {
        let info = release_info_for(
            release_with(&LINUX_ASSETS),
            "0.1.0",
            "linux",
            "x86_64",
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(info.zip_name, "Corvene-0.2.0-x86_64.AppImage");
        let info = release_info_for(
            release_with(&LINUX_ASSETS),
            "0.1.0",
            "linux",
            "aarch64",
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(info.zip_name, "Corvene-0.2.0-aarch64.AppImage");
        // the 32-bit builds, by their AppImage names
        for (arch, name) in [
            ("x86", "Corvene-0.2.0-i686.AppImage"),
            ("arm", "Corvene-0.2.0-armhf.AppImage"),
        ] {
            let info = release_info_for(release_with(&LINUX_ASSETS), "0.1.0", "linux", arch, false)
                .unwrap()
                .unwrap();
            assert_eq!(info.zip_name, name);
        }
        // macOS still takes the zip from the same release
        let info = release_info_for(
            release_with(&LINUX_ASSETS),
            "0.1.0",
            "macos",
            "aarch64",
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(info.zip_name, "Corvene-0.2.0-macos-universal.zip");
    }

    #[test]
    fn windows_picks_the_installer_for_its_architecture() {
        let assets = [
            "Corvene-Full-0.2.0-x86_64-setup.exe",
            "Corvene-0.2.0-aarch64-setup.exe",
            "Corvene-0.2.0-x86_64-setup.exe",
            "Corvene-0.2.0-i686-setup.exe",
            "Corvene-0.2.0-x86_64.AppImage",
        ];
        for (arch, name) in [
            ("x86_64", "Corvene-0.2.0-x86_64-setup.exe"),
            ("aarch64", "Corvene-0.2.0-aarch64-setup.exe"),
            ("x86", "Corvene-0.2.0-i686-setup.exe"),
        ] {
            let info = release_info_for(release_with(&assets), "0.1.0", "windows", arch, false)
                .unwrap()
                .unwrap();
            assert_eq!(info.zip_name, name);
        }
    }

    /// Every kind of asset a release carries, both variants.
    const ALL_ASSETS: [&str; 22] = [
        "Corvene-0.2.0-windows-x86_64-portable.zip",
        "Corvene-Full-0.2.0-windows-x86_64-portable.zip",
        "Corvene-0.2.0-macos-universal.zip",
        "Corvene-0.2.0-macos-arm64.zip",
        "Corvene-Full-0.2.0-macos-universal.zip",
        "Corvene-Full-0.2.0-macos-universal.dmg",
        "Corvene-0.2.0-x86_64.msi",
        "Corvene-0.2.0-x86_64-setup.exe",
        "Corvene-Full-0.2.0-x86_64.msi",
        "Corvene-Full-0.2.0-x86_64-setup.exe",
        "Corvene-0.2.0-linux-x86_64.tar.gz",
        "Corvene-Full-0.2.0-linux-x86_64.tar.gz",
        "Corvene-0.2.0-x86_64.flatpak",
        "Corvene-0.2.0-x86_64.snap",
        "Corvene-0.2.0-x86_64.AppImage",
        "Corvene-Full-0.2.0-x86_64.AppImage",
        "corvene-0.2.0-1.x86_64.rpm",
        "corvene-full-0.2.0-1.x86_64.rpm",
        "corvene_0.2.0_amd64.deb",
        "corvene-full_0.2.0_amd64.deb",
        "Corvene-0.2.0-android-foss-arm64.apk",
        "Corvene-Full-0.2.0-android-foss-arm64.apk",
    ];

    #[test]
    fn each_variant_updates_to_its_own_asset() {
        for (os, arch, full, name) in [
            (
                "macos",
                "aarch64",
                false,
                "Corvene-0.2.0-macos-universal.zip",
            ),
            (
                "macos",
                "aarch64",
                true,
                "Corvene-Full-0.2.0-macos-universal.zip",
            ),
            ("windows", "x86_64", false, "Corvene-0.2.0-x86_64-setup.exe"),
            (
                "windows",
                "x86_64",
                true,
                "Corvene-Full-0.2.0-x86_64-setup.exe",
            ),
            ("linux", "x86_64", false, "Corvene-0.2.0-x86_64.AppImage"),
            (
                "linux",
                "x86_64",
                true,
                "Corvene-Full-0.2.0-x86_64.AppImage",
            ),
        ] {
            let info = release_info_for(release_with(&ALL_ASSETS), "0.1.0", os, arch, full)
                .unwrap()
                .unwrap();
            assert_eq!(info.zip_name, name, "{os} {arch} full={full}");
        }
        // a Windows portable zip is never a macOS update
        let windows_only = release_with(&["Corvene-0.2.0-windows-x86_64-portable.zip"]);
        assert!(matches!(
            release_info_for(windows_only, "0.1.0", "macos", "x86_64", false),
            Err(UpdateError::NoAsset(_))
        ));
    }

    #[test]
    fn linux_ignores_the_deb_and_other_architectures() {
        let only_deb = release_with(&[
            "corvene_0.2.0_amd64.deb",
            "Corvene-0.2.0-macos-universal.zip",
        ]);
        assert!(matches!(
            release_info_for(only_deb, "0.1.0", "linux", "x86_64", false),
            Err(UpdateError::NoAsset(_))
        ));
        let other_arch = release_with(&["Corvene-0.2.0-aarch64.AppImage"]);
        assert!(matches!(
            release_info_for(other_arch, "0.1.0", "linux", "x86_64", false),
            Err(UpdateError::NoAsset(_))
        ));
        let mut no_digest = release_with(&["Corvene-0.2.0-x86_64.AppImage"]);
        no_digest.assets[0].digest = None;
        assert!(matches!(
            release_info_for(no_digest, "0.1.0", "linux", "x86_64", false),
            Err(UpdateError::NoDigest(_))
        ));
        let full = [asset("Corvene-Full-0.2.0-x86_64.AppImage")];
        assert!(pick_asset(&full, "linux", "x86_64", false).is_none());
    }
}

#[cfg(all(test, not(any(target_os = "macos", windows))))]
mod linux_tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// sha256 of "corvene\n"
    const SHA: &str = "d685a1bffc4dd7a8a6e5699d2c994b5628f7fdc9425f56254f9b0a12c8ed14aa";

    /// `<tmp>/apps/Corvene.AppImage` (old, `0644`) and a downloaded
    /// `<tmp>/updates/Corvene-9.9.9-x86_64.AppImage`.
    fn setup(new_contents: &[u8]) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path().join("apps");
        let updates = tmp.path().join("updates");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::create_dir_all(&updates).unwrap();
        let running = apps.join("Corvene.AppImage");
        std::fs::write(&running, b"old image").unwrap();
        std::fs::set_permissions(&running, std::fs::Permissions::from_mode(0o644)).unwrap();
        let new_image = updates.join("Corvene-9.9.9-x86_64.AppImage");
        std::fs::write(&new_image, new_contents).unwrap();
        (tmp, running, new_image)
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != "Corvene.AppImage")
            .collect()
    }

    #[test]
    fn installs_a_verified_appimage_by_rename() {
        let (_tmp, running, new_image) = setup(b"corvene\n");
        install_appimage(&new_image, &running, SHA).unwrap();
        assert_eq!(std::fs::read(&running).unwrap(), b"corvene\n");
        let mode = std::fs::metadata(&running).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert!(leftovers(running.parent().unwrap()).is_empty());
        // the download stays where it was (the cache is cleared next time)
        assert!(new_image.exists());
    }

    #[test]
    fn a_failed_verification_leaves_the_running_image_alone() {
        let (_tmp, running, new_image) = setup(b"corvene!\n");
        assert!(matches!(
            install_appimage(&new_image, &running, SHA),
            Err(UpdateError::Checksum(_))
        ));
        assert_eq!(std::fs::read(&running).unwrap(), b"old image");
        let mode = std::fs::metadata(&running).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
        assert!(leftovers(running.parent().unwrap()).is_empty());
    }

    #[test]
    fn appimage_target_needs_a_replaceable_regular_file() {
        let (tmp, running, _) = setup(b"x");
        assert_eq!(
            appimage_target(Some(running.clone().into_os_string())),
            Some(std::fs::canonicalize(&running).unwrap())
        );
        // a link to the image resolves to the image
        let link = tmp.path().join("corvene");
        std::os::unix::fs::symlink(&running, &link).unwrap();
        assert_eq!(
            appimage_target(Some(link.into_os_string())),
            Some(std::fs::canonicalize(&running).unwrap())
        );
        assert_eq!(appimage_target(None), None);
        assert_eq!(appimage_target(Some("".into())), None);
        assert_eq!(appimage_target(Some("Corvene.AppImage".into())), None);
        assert_eq!(appimage_target(Some(tmp.path().join("apps").into())), None);
        assert_eq!(
            appimage_target(Some(tmp.path().join("missing").into())),
            None
        );
    }
}
