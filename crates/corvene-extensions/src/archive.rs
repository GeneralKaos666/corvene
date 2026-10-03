//! Extracting the archives extensions arrive in (`.vsix`, `.zip`,
//! `.sublime-package`, `.tar.gz`), entry by entry under a policy fit for
//! untrusted input: no path may leave the target folder, no symlinks or
//! device entries, caps on entry count and sizes. The system's `ditto` and
//! `tar` (which the signed packs use) enforce none of that.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::ExtensionError;

/// Caps on what one archive may unpack to.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_entry_bytes: 64 * 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
            max_depth: 32,
        }
    }
}

/// What an archive is, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zip,
    TarGz,
    Tar,
}

impl Kind {
    /// `None` when `path` is not an archive this crate opens.
    pub fn sniff(path: &Path) -> Result<Option<Self>, ExtensionError> {
        let mut file = std::fs::File::open(path)?;
        let mut head = [0u8; 512];
        let mut read = 0;
        while read < head.len() {
            let n = file.read(&mut head[read..])?;
            if n == 0 {
                break;
            }
            read += n;
        }
        let head = &head[..read];
        Ok(
            if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
                Some(Kind::Zip)
            } else if head.starts_with(&[0x1f, 0x8b]) {
                Some(Kind::TarGz)
            } else if head.len() >= 262 && &head[257..262] == b"ustar" {
                Some(Kind::Tar)
            } else {
                None
            },
        )
    }
}

/// What came out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    pub files: usize,
    pub bytes: u64,
}

/// Unpack `archive` into `into` (created if missing).
pub fn extract(archive: &Path, into: &Path, limits: &Limits) -> Result<Extracted, ExtensionError> {
    let kind = Kind::sniff(archive)?.ok_or_else(|| {
        ExtensionError::Archive(format!("{} is not a zip or tar archive", archive.display()))
    })?;
    std::fs::create_dir_all(into)?;
    let file = std::fs::File::open(archive)?;
    match kind {
        Kind::Zip => extract_zip(file, into, limits),
        Kind::TarGz => extract_tar(flate2::read::GzDecoder::new(file), into, limits),
        Kind::Tar => extract_tar(file, into, limits),
    }
}

/// `name` as a path under the target, or why it is refused.
pub fn safe_relative(name: &str, limits: &Limits) -> Result<Option<PathBuf>, ExtensionError> {
    if name.contains('\0') {
        return Err(ExtensionError::Archive(
            "an entry name holds a NUL byte".to_string(),
        ));
    }
    if name.contains('\\') {
        return Err(ExtensionError::Archive(format!(
            "entry {name:?} uses backslashes"
        )));
    }
    let path = Path::new(name);
    let mut out = PathBuf::new();
    let mut depth = 0;
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ExtensionError::Archive(format!(
                    "entry {name:?} leaves the folder"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ExtensionError::Archive(format!(
                    "entry {name:?} is an absolute path"
                )));
            }
        }
    }
    if depth > limits.max_depth {
        return Err(ExtensionError::Archive(format!(
            "entry {name:?} nests too deeply"
        )));
    }
    Ok((!out.as_os_str().is_empty()).then_some(out))
}

struct Budget<'a> {
    limits: &'a Limits,
    entries: usize,
    bytes: u64,
}

impl Budget<'_> {
    fn entry(&mut self) -> Result<(), ExtensionError> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(ExtensionError::Archive(format!(
                "the archive has more than {} entries",
                self.limits.max_entries
            )));
        }
        Ok(())
    }

    fn size(&mut self, declared: u64, name: &str) -> Result<(), ExtensionError> {
        if declared > self.limits.max_entry_bytes {
            return Err(ExtensionError::Archive(format!(
                "entry {name:?} is too large"
            )));
        }
        self.bytes = self.bytes.saturating_add(declared);
        if self.bytes > self.limits.max_total_bytes {
            return Err(ExtensionError::Archive(
                "the archive unpacks to too much data".to_string(),
            ));
        }
        Ok(())
    }
}

/// Copy `reader` to `path`, refusing more than `max` bytes (a zip entry's
/// declared size is not trusted).
fn write_capped(mut reader: impl Read, path: &Path, max: u64) -> Result<u64, ExtensionError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::File::create(path)?;
    let written = std::io::copy(&mut (&mut reader).take(max + 1), &mut file)?;
    if written > max {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(ExtensionError::Archive(format!(
            "{} is larger than declared",
            path.display()
        )));
    }
    Ok(written)
}

fn extract_zip(
    file: std::fs::File,
    into: &Path,
    limits: &Limits,
) -> Result<Extracted, ExtensionError> {
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| ExtensionError::Archive(format!("not a readable zip: {err}")))?;
    let mut budget = Budget {
        limits,
        entries: 0,
        bytes: 0,
    };
    let mut out = Extracted::default();
    for index in 0..archive.len() {
        budget.entry()?;
        let mut entry = archive
            .by_index(index)
            .map_err(|err| ExtensionError::Archive(format!("zip entry {index}: {err}")))?;
        let name = entry.name().to_string();
        if entry.is_symlink() {
            return Err(ExtensionError::Archive(format!(
                "entry {name:?} is a symlink"
            )));
        }
        let Some(relative) = safe_relative(&name, limits)? else {
            continue;
        };
        let target = into.join(relative);
        if entry.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if entry.encrypted() {
            return Err(ExtensionError::Archive(format!(
                "entry {name:?} is encrypted"
            )));
        }
        budget.size(entry.size(), &name)?;
        out.bytes += write_capped(&mut entry, &target, limits.max_entry_bytes)?;
        out.files += 1;
    }
    Ok(out)
}

fn extract_tar(
    reader: impl Read,
    into: &Path,
    limits: &Limits,
) -> Result<Extracted, ExtensionError> {
    let mut archive = tar::Archive::new(reader);
    let mut budget = Budget {
        limits,
        entries: 0,
        bytes: 0,
    };
    let mut out = Extracted::default();
    let entries = archive
        .entries()
        .map_err(|err| ExtensionError::Archive(format!("not a readable tar: {err}")))?;
    for entry in entries {
        budget.entry()?;
        let mut entry =
            entry.map_err(|err| ExtensionError::Archive(format!("tar entry: {err}")))?;
        let path = entry
            .path()
            .map_err(|err| ExtensionError::Archive(format!("tar entry name: {err}")))?
            .to_string_lossy()
            .into_owned();
        use tar::EntryType::*;
        match entry.header().entry_type() {
            Regular | Continuous | GNUSparse => {}
            Directory => {
                if let Some(relative) = safe_relative(&path, limits)? {
                    std::fs::create_dir_all(into.join(relative))?;
                }
                continue;
            }
            // pax and long-name records are consumed by the reader itself
            XHeader | XGlobalHeader | GNULongName | GNULongLink => continue,
            Symlink | Link => {
                return Err(ExtensionError::Archive(format!("entry {path:?} is a link")));
            }
            _ => {
                return Err(ExtensionError::Archive(format!(
                    "entry {path:?} is not a file"
                )));
            }
        }
        let Some(relative) = safe_relative(&path, limits)? else {
            continue;
        };
        budget.size(entry.size(), &path)?;
        out.bytes += write_capped(&mut entry, &into.join(relative), limits.max_entry_bytes)?;
        out.files += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(entries: &[(&str, &[u8])], symlink: Option<(&str, &str)>) -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        let path = dir.join("a.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).expect("start");
            writer.write_all(data).expect("write");
        }
        if let Some((name, target)) = symlink {
            writer.add_symlink(name, target, options).expect("symlink");
        }
        writer.finish().expect("finish");
        path
    }

    fn tar_gz_with(entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        let path = dir.join("a.tar.gz");
        let file = std::fs::File::create(&path).expect("create");
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(gz);
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            // written raw: the builder itself refuses `..`, hostile archives do not
            let gnu = header.as_gnu_mut().expect("gnu header");
            gnu.name[..name.len()].copy_from_slice(name.as_bytes());
            header.set_cksum();
            builder.append(&header, *data).expect("append");
        }
        builder.into_inner().expect("tar").finish().expect("gz");
        path
    }

    #[test]
    fn zip_and_tar_round_trip() {
        let limits = Limits::default();
        for archive in [
            zip_with(
                &[
                    ("extension/package.json", b"{}"),
                    ("extension/syntaxes/a.json", b"[]"),
                    ("dir/", b""),
                ],
                None,
            ),
            tar_gz_with(&[
                ("repo-abc/extension/package.json", b"{}"),
                ("repo-abc/extension/syntaxes/a.json", b"[]"),
            ]),
        ] {
            let into = tempfile::tempdir().expect("tempdir");
            let kind = Kind::sniff(&archive).expect("sniff").expect("an archive");
            assert!(matches!(kind, Kind::Zip | Kind::TarGz));
            let out = extract(&archive, into.path(), &limits).expect("extract");
            assert_eq!(out.files, 2);
            assert_eq!(out.bytes, 4);
            let found: Vec<_> = walkdir(into.path());
            assert!(
                found.iter().any(|p| p.ends_with("package.json")),
                "{found:?}"
            );
        }
    }

    fn walkdir(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                out.extend(walkdir(&path));
            } else {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn hostile_entries_are_refused() {
        let limits = Limits::default();
        let cases: Vec<(PathBuf, &str)> = vec![
            (zip_with(&[("../escape.txt", b"x")], None), "leaves"),
            (zip_with(&[("/abs.txt", b"x")], None), "absolute"),
            (zip_with(&[("a\\b.txt", b"x")], None), "backslash"),
            (
                zip_with(&[("ok.txt", b"x")], Some(("link", "/etc/passwd"))),
                "symlink",
            ),
            (tar_gz_with(&[("../up.txt", b"x")]), "leaves"),
        ];
        for (archive, expected) in cases {
            let into = tempfile::tempdir().expect("tempdir");
            let err = extract(&archive, into.path(), &limits).expect_err("refused");
            assert!(err.to_string().contains(expected), "{err}");
            assert!(
                walkdir(into.path())
                    .iter()
                    .all(|p| !p.to_string_lossy().contains("escape"))
            );
        }
        let deep = format!("{}x", "d/".repeat(40));
        let err = extract(
            &zip_with(&[(deep.as_str(), b"x")], None),
            tempfile::tempdir().expect("t").path(),
            &limits,
        )
        .expect_err("depth");
        assert!(err.to_string().contains("deeply"), "{err}");
    }

    #[test]
    fn caps_apply() {
        let tight = Limits {
            max_entries: 2,
            max_entry_bytes: 3,
            max_total_bytes: 5,
            max_depth: 32,
        };
        let too_many = zip_with(&[("a", b"1"), ("b", b"2"), ("c", b"3")], None);
        let err =
            extract(&too_many, tempfile::tempdir().expect("t").path(), &tight).expect_err("count");
        assert!(err.to_string().contains("entries"), "{err}");
        let too_big = zip_with(&[("a", b"1234")], None);
        let err =
            extract(&too_big, tempfile::tempdir().expect("t").path(), &tight).expect_err("size");
        assert!(err.to_string().contains("too large"), "{err}");
        let total = zip_with(&[("a", b"123"), ("b", b"123")], None);
        let err =
            extract(&total, tempfile::tempdir().expect("t").path(), &tight).expect_err("total");
        assert!(err.to_string().contains("too much"), "{err}");
    }

    #[test]
    fn sniff_rejects_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("x.json");
        std::fs::write(&path, b"{\"a\": 1}").expect("write");
        assert_eq!(Kind::sniff(&path).expect("sniff"), None);
        assert!(extract(&path, dir.path(), &Limits::default()).is_err());
    }
}
