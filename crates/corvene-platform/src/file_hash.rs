//! GHD `getFileHash(path, type)` (`app/src/lib/get-file-hash.ts`): the hex
//! digest of a file on disk, read in chunks. GitHub Desktop hashes its
//! bundle and SSH keys with it (sha256 only, though it also offers sha1);
//! Corvene checks downloaded updates ([`crate::updater::verify`]) and
//! component packs (`corvene_packs::sha256_file`).

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

/// GHD `getFileHash`'s `type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
}

/// GHD `getFileHash(path, type)`: the lower-case hex digest of the file at
/// `path`. A missing file is an `io::Error` of kind `NotFound` (GHD's
/// rejection with `code: 'ENOENT'`).
pub fn get_file_hash(path: &Path, algorithm: HashAlgorithm) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    match algorithm {
        HashAlgorithm::Sha1 => {
            let mut hasher = sha1_smol::Sha1::new();
            read_chunks(&mut file, |chunk| hasher.update(chunk))?;
            Ok(hasher.digest().to_string())
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            read_chunks(&mut file, |chunk| hasher.update(chunk))?;
            Ok(format!("{:x}", hasher.finalize()))
        }
    }
}

/// Feed `file` to `update` in 256 KiB chunks (GHD pipes a read stream).
fn read_chunks(file: &mut std::fs::File, mut update: impl FnMut(&[u8])) -> std::io::Result<()> {
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        update(&buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty");
        std::fs::write(&path, "").unwrap();
        assert_eq!(
            get_file_hash(&path, HashAlgorithm::Sha1).unwrap(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(
            get_file_hash(&path, HashAlgorithm::Sha256).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
