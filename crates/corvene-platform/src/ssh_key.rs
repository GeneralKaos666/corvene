//! The SSH key in `~/.ssh` that git's ssh uses: create one with `ssh-keygen`,
//! import one (Android), read its public half and, on the desktop, hand it
//! to the running `ssh-agent`.
//!
//! Android keeps the key in the app-private storage for the bundled OpenSSH
//! client (`crate::android`); the desktop's key helper (Corvene
//! `350-ssh-key-helper`) uses the system's OpenSSH and the user's own
//! `~/.ssh`, where an existing key is never replaced. GitHub Desktop has
//! no key helper: it points to GitHub's documentation.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The names ssh tries by itself, by key type.
const SSH_KEY_NAMES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// `~/.ssh`; `CORVENE_SSH_DIR` stands in for it (tests and the parity
/// harness, which must not show or touch the user's keys).
fn ssh_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CORVENE_SSH_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::home_dir()?.join(".ssh"))
}

/// The key in use: the first of [`SSH_KEY_NAMES`] that exists (a created
/// key is `id_ed25519`).
pub fn ssh_key_file() -> Option<PathBuf> {
    let dir = ssh_dir()?;
    SSH_KEY_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|file| file.exists())
        .or_else(|| Some(dir.join(SSH_KEY_NAMES[0])))
}

/// Android: the `ssh-keygen` bundled beside git; elsewhere the system's.
fn ssh_tool(name: &str) -> Result<PathBuf, String> {
    if cfg!(target_os = "android") {
        return std::env::var_os("GIT_EXEC_PATH")
            .map(|bin| PathBuf::from(bin).join(name))
            .filter(|tool| tool.exists())
            .ok_or_else(|| format!("this build has no {name}"));
    }
    Ok(PathBuf::from(name))
}

/// `chmod`: ssh refuses keys others can read.
fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Why [`import_ssh_key`] failed.
#[derive(Debug, PartialEq, Eq)]
pub enum SshImportError {
    /// The key is encrypted and the passphrase is missing or wrong.
    Passphrase,
    Other(String),
}

impl From<String> for SshImportError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

/// The file name ssh expects for a public key line's type.
fn ssh_key_name(public: &str) -> Option<&'static str> {
    match public.split_whitespace().next()? {
        "ssh-ed25519" => Some("id_ed25519"),
        "ssh-rsa" => Some("id_rsa"),
        kind if kind.starts_with("ecdsa-sha2-") => Some("id_ecdsa"),
        _ => None,
    }
}

/// Makes the private key in `source` the key ssh uses and returns its
/// public key. An encrypted key needs its `passphrase`; the copy in the
/// app-private storage is stored without one, like a created key (nothing
/// could ask for it during a fetch in the background). A key already there
/// is kept beside it as `<name>.replaced`. Android only: the desktop's
/// `~/.ssh` belongs to the user.
pub fn import_ssh_key(source: &Path, passphrase: Option<&str>) -> Result<String, SshImportError> {
    const LIMIT: u64 = 64 * 1024;
    let io = |err: std::io::Error| SshImportError::Other(err.to_string());
    let length = std::fs::metadata(source).map_err(io)?.len();
    let text = std::fs::read(source).map_err(io)?;
    if length > LIMIT || !String::from_utf8_lossy(&text).contains("PRIVATE KEY-----") {
        return Err(SshImportError::Other(
            "This file is not an SSH private key.".to_string(),
        ));
    }
    let keygen = ssh_tool("ssh-keygen")?;
    let dir = ssh_dir().ok_or_else(|| "no home directory".to_string())?;
    std::fs::create_dir_all(&dir).map_err(io)?;
    let _ = set_mode(&dir, 0o700);
    let staged = dir.join("import.tmp");
    std::fs::write(&staged, &text).map_err(io)?;
    set_mode(&staged, 0o600).map_err(io)?;
    let result = (|| {
        let passphrase = passphrase.unwrap_or("");
        let run = |args: &[&str]| {
            std::process::Command::new(&keygen)
                .args(args)
                .arg(&staged)
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(io)
        };
        let public = run(&["-y", "-P", passphrase, "-f"])?;
        if !public.status.success() {
            let message = String::from_utf8_lossy(&public.stderr).trim().to_string();
            return Err(if message.contains("passphrase") {
                SshImportError::Passphrase
            } else {
                SshImportError::Other(message)
            });
        }
        let public = String::from_utf8_lossy(&public.stdout).trim().to_string();
        let name = ssh_key_name(&public).ok_or_else(|| {
            SshImportError::Other("Corvene's ssh does not use this kind of key.".to_string())
        })?;
        if !passphrase.is_empty() {
            let plain = run(&["-q", "-p", "-P", passphrase, "-N", "", "-f"])?;
            if !plain.status.success() {
                return Err(SshImportError::Other(
                    String::from_utf8_lossy(&plain.stderr).trim().to_string(),
                ));
            }
        }
        for old in SSH_KEY_NAMES.iter().map(|name| dir.join(name)) {
            if old.exists() {
                let _ = std::fs::rename(&old, old.with_extension("replaced"));
                let _ = std::fs::rename(
                    old.with_extension("pub"),
                    old.with_extension("replaced.pub"),
                );
            }
        }
        let file = dir.join(name);
        std::fs::rename(&staged, &file).map_err(io)?;
        std::fs::write(file.with_extension("pub"), format!("{public}\n")).map_err(io)?;
        Ok(public)
    })();
    let _ = std::fs::remove_file(&staged);
    // the picked copy is needed again once the passphrase is known
    if result != Err(SshImportError::Passphrase) {
        let _ = std::fs::remove_file(source);
    }
    forget_public_key();
    result
}

static SSH_PUBLIC_KEY: Mutex<Option<Option<String>>> = Mutex::new(None);

/// The public key of the key in `~/.ssh` (read once, then remembered until
/// [`forget_public_key`]).
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

/// Read the key again next time (it may have been made outside Corvene).
pub fn forget_public_key() {
    if let Ok(mut cached) = SSH_PUBLIC_KEY.lock() {
        *cached = None;
    }
}

/// Creates `~/.ssh/id_ed25519` with `comment` and `passphrase` (`None` or
/// empty: none) and returns its public key. An existing key is kept and
/// its public key returned.
///
/// `ssh-keygen` only reads a passphrase from a terminal or its `-N`
/// argument, so it is passed there.
pub fn create_ssh_key(comment: &str, passphrase: Option<&str>) -> Result<String, String> {
    forget_public_key();
    if let Some(key) = ssh_public_key() {
        return Ok(key);
    }
    let file = ssh_key_file().ok_or("no home directory")?;
    let dir = file.parent().ok_or("no home directory")?;
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let _ = set_mode(dir, 0o700);
    let keygen = ssh_tool("ssh-keygen")?;
    let output = std::process::Command::new(keygen)
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            passphrase.unwrap_or(""),
            "-C",
            comment,
            "-f",
        ])
        .arg(&file)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("could not run ssh-keygen: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    forget_public_key();
    ssh_public_key().ok_or_else(|| "ssh-keygen wrote no public key".to_string())
}

/// The passphrase prompt `ssh-add` runs (`SSH_ASKPASS`): it prints the
/// passphrase from the environment, so it never sits in a file.
#[cfg(unix)]
const ASKPASS_SCRIPT: &str = "#!/bin/sh\nprintf '%s\\n' \"$CORVENE_SSH_PASSPHRASE\"\n";

/// Hands the key in `file` to the running `ssh-agent` (`ssh-add`); on macOS
/// with `--apple-use-keychain`, which also stores the passphrase in the
/// login keychain. A `passphrase` is answered through a one-off
/// `SSH_ASKPASS` script.
pub fn add_to_agent(file: &Path, passphrase: Option<&str>) -> Result<(), String> {
    if cfg!(target_os = "android") {
        return Err("Android has no ssh-agent".to_string());
    }
    if std::env::var_os("SSH_AUTH_SOCK").is_none_or(|s| s.is_empty()) && !cfg!(windows) {
        return Err("No ssh-agent is running for Corvene (SSH_AUTH_SOCK is not set).".to_string());
    }
    let mut command = std::process::Command::new(ssh_tool("ssh-add")?);
    if cfg!(target_os = "macos") {
        command.arg("--apple-use-keychain");
    }
    command.arg(file).stdin(std::process::Stdio::null());
    let passphrase = passphrase.filter(|p| !p.is_empty());
    #[cfg(unix)]
    let askpass = match passphrase {
        Some(passphrase) => {
            let dir = std::env::temp_dir().join(format!("corvene-askpass-{}", std::process::id()));
            std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
            let _ = set_mode(&dir, 0o700);
            let script = dir.join("ssh-askpass");
            std::fs::write(&script, ASKPASS_SCRIPT).map_err(|err| err.to_string())?;
            set_mode(&script, 0o700).map_err(|err| err.to_string())?;
            command
                .env("SSH_ASKPASS", &script)
                .env("SSH_ASKPASS_REQUIRE", "force")
                // OpenSSH before 8.4 only asks with a display set
                .env(
                    "DISPLAY",
                    std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()),
                )
                .env("CORVENE_SSH_PASSPHRASE", passphrase);
            Some(dir)
        }
        None => None,
    };
    #[cfg(not(unix))]
    if passphrase.is_some() {
        return Err(
            "Add a key with a passphrase to the agent with ssh-add in a terminal.".to_string(),
        );
    }
    let output = command.output();
    #[cfg(unix)]
    if let Some(dir) = askpass {
        let _ = std::fs::remove_dir_all(dir);
    }
    let output = output.map_err(|err| format!("could not run ssh-add: {err}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if message.is_empty() {
            "ssh-add failed".to_string()
        } else {
            message
        });
    }
    Ok(())
}

/// The lines [`remember_in_keychain`] appends to `~/.ssh/config`.
const KEYCHAIN_CONFIG: &str = "\n# Added by Corvene: load SSH keys and their passphrases from the \
                               keychain\nHost *\n  AddKeysToAgent yes\n  UseKeychain yes\n";

/// macOS: makes ssh load keys and their passphrases from the keychain after
/// a restart (`UseKeychain`, `AddKeysToAgent` for every host in
/// `~/.ssh/config`), unless the file already sets `UseKeychain`. Returns
/// whether it wrote anything.
pub fn remember_in_keychain() -> Result<bool, String> {
    let dir = ssh_dir().ok_or("no home directory")?;
    let config = dir.join("config");
    let existing = std::fs::read_to_string(&config).unwrap_or_default();
    if existing.lines().any(|line| {
        line.trim_start()
            .to_ascii_lowercase()
            .starts_with("usekeychain")
    }) {
        return Ok(false);
    }
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(KEYCHAIN_CONFIG);
    std::fs::write(&config, text).map_err(|err| err.to_string())?;
    let _ = set_mode(&config, 0o600);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keys_by_type() {
        assert_eq!(ssh_key_name("ssh-ed25519 AAAA x"), Some("id_ed25519"));
        assert_eq!(ssh_key_name("ecdsa-sha2-nistp256 AAAA"), Some("id_ecdsa"));
        assert_eq!(ssh_key_name("ssh-dss AAAA"), None);
    }

    #[test]
    fn keychain_config_is_a_host_block() {
        assert!(KEYCHAIN_CONFIG.contains("\nHost *\n  AddKeysToAgent yes\n  UseKeychain yes\n"));
    }
}
