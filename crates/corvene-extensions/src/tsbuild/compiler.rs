//! The C compiler a grammar build uses: `CORVENE_CC` / `CORVENE_CXX`, else
//! the toolchain `xcrun` finds (the Xcode Command Line Tools), else `cc`
//! and `c++` on the PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A usable toolchain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compiler {
    pub cc: PathBuf,
    pub cxx: Option<PathBuf>,
    /// `cc --version`'s first line
    pub version: String,
    /// macOS: the SDK (`xcrun --show-sdk-path`), passed as `-isysroot`
    pub sysroot: Option<PathBuf>,
}

impl Compiler {
    /// Find one, or `None` with a hint for the consent sheet.
    pub fn find() -> Result<Self, String> {
        // macOS: /usr/bin/cc is Apple's shim that picks the SDK itself;
        // the toolchain's own clang needs the sysroot spelled out
        let shim = |name: &str| {
            let p = PathBuf::from("/usr/bin").join(name);
            (cfg!(target_os = "macos") && p.is_file()).then_some(p)
        };
        let cc = std::env::var_os("CORVENE_CC")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| shim("cc"))
            .or_else(|| xcrun_find("clang"))
            .or_else(|| which("cc"))
            .or_else(|| which("clang"))
            .or_else(|| which("gcc"))
            .ok_or_else(|| {
                if cfg!(target_os = "macos") {
                    "No C compiler was found. Install the Xcode Command Line Tools (run xcode-select --install in Terminal) and try again.".to_string()
                } else {
                    "No C compiler was found. Install clang or gcc and try again.".to_string()
                }
            })?;
        let cxx = std::env::var_os("CORVENE_CXX")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| shim("c++"))
            .or_else(|| xcrun_find("clang++"))
            .or_else(|| which("c++"))
            .or_else(|| which("clang++"))
            .or_else(|| which("g++"));
        let version = Command::new(&cc)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "version unknown".to_string());
        // on a Mac without the Command Line Tools /usr/bin/cc is a stub that
        // offers to install them: its --version fails
        if version == "version unknown" && cfg!(target_os = "macos") {
            return Err("The C compiler at /usr/bin/cc cannot run: install the Xcode Command Line Tools (run xcode-select --install in Terminal) and try again.".to_string());
        }
        Ok(Self {
            cc,
            cxx,
            version,
            sysroot: sdk_path(),
        })
    }

    /// Target flags: the deployment target and SDK on macOS.
    pub fn target_flags(&self) -> Vec<String> {
        if cfg!(target_os = "macos") {
            let mut flags = vec![format!("-mmacosx-version-min={}", deployment_target())];
            if let Some(sysroot) = &self.sysroot {
                flags.push("-isysroot".into());
                flags.push(sysroot.to_string_lossy().into_owned());
            }
            flags
        } else {
            vec!["-ffunction-sections".into(), "-fdata-sections".into()]
        }
    }

    /// Link flags for a shared library at `out`.
    pub fn link_flags(&self, out: &Path) -> Vec<String> {
        let name = out
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if cfg!(target_os = "macos") {
            let mut flags = vec![
                "-dynamiclib".to_string(),
                "-Wl,-dead_strip".to_string(),
                "-Wl,-x".to_string(),
            ];
            flags.extend(self.target_flags());
            flags.push("-install_name".into());
            flags.push(format!("@rpath/{name}"));
            flags
        } else if cfg!(windows) {
            vec!["-shared".into()]
        } else {
            vec![
                "-shared".into(),
                "-Wl,--gc-sections".into(),
                "-Wl,-s".into(),
                format!("-Wl,-soname,{name}"),
            ]
        }
    }
}

/// The app's `LSMinimumSystemVersion` (packaging/Info.plist), so a built
/// library loads on every Mac the app does.
fn deployment_target() -> String {
    std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".to_string())
}

/// `xcrun --show-sdk-path` (macOS).
fn sdk_path() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = Command::new("/usr/bin/xcrun")
        .args(["--show-sdk-path"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    path.is_dir().then_some(path)
}

fn xcrun_find(tool: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = Command::new("/usr/bin/xcrun")
        .args(["--find", tool])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    path.is_file().then_some(path)
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}
