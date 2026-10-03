//! Port of GitHub Desktop's `app/test/unit/get-file-hash-test.ts`.
//!
//! Corvene equivalent: `getFileHash(path, 'sha256')`
//! (`lib/get-file-hash.ts`, the hex digest of a file on disk; GitHub
//! Desktop hashes SSH keys and its bundle with it) is
//! `corvene_packs::sha256_file(path)` (the hex sha256 of a file, used for
//! the optional component packs). Corvene has no sha1 file hash, so
//! `getFileHash(path, 'sha1')` is a stand-in ([`get_file_hash_sha1`]).
//! GitHub Desktop's rejection with `code: 'ENOENT'` is an `io::Error` of
//! kind `NotFound`.

use std::path::Path;

use corvene_packs::sha256_file;

/// Stand-in for GitHub Desktop's `getFileHash(path, 'sha1')`
/// (`lib/get-file-hash.ts`). Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`.
fn get_file_hash_sha1(_path: &Path) -> std::io::Result<String> {
    unimplemented!("Corvene has no sha1 file hash (corvene_packs::sha256_file only)")
}

/// `mkdtemp(path.join(tmpdir(), 'hash-test-'))`, removed when dropped
/// (GitHub Desktop's `rm(dir, { recursive: true })`).
fn mkdtemp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("hash-test-")
        .tempdir()
        .expect("create a temporary directory")
}

fn write_file(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

// GHD: unit/get-file-hash-test.ts › get-file-hash › returns consistent sha256 hash for known content
#[test]
fn returns_consistent_sha256_hash_for_known_content() {
    let dir = mkdtemp();
    let file_path = dir.path().join("test-file.js");

    write_file(&file_path, "hello world");

    let hash = sha256_file(&file_path).expect("getFileHash");

    // SHA-256 of "hello world"
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

// GHD: unit/get-file-hash-test.ts › get-file-hash › returns different hashes for different content
#[test]
fn returns_different_hashes_for_different_content() {
    let dir = mkdtemp();
    let file1 = dir.path().join("file1.js");
    let file2 = dir.path().join("file2.js");

    write_file(&file1, "original content");
    write_file(&file2, "modified content");

    let hash1 = sha256_file(&file1).expect("getFileHash");
    let hash2 = sha256_file(&file2).expect("getFileHash");

    assert_ne!(hash1, hash2);
}

// GHD: unit/get-file-hash-test.ts › get-file-hash › returns same hash for same content in different files
#[test]
fn returns_same_hash_for_same_content_in_different_files() {
    let dir = mkdtemp();
    let file1 = dir.path().join("file1.js");
    let file2 = dir.path().join("file2.js");

    write_file(&file1, "identical content");
    write_file(&file2, "identical content");

    let hash1 = sha256_file(&file1).expect("getFileHash");
    let hash2 = sha256_file(&file2).expect("getFileHash");

    assert_eq!(hash1, hash2);
}

// GHD: unit/get-file-hash-test.ts › get-file-hash › rejects for non-existent file
#[test]
fn rejects_for_non_existent_file() {
    let result = sha256_file(Path::new("/nonexistent/path/file.js"));
    let err = result.expect_err("getFileHash should reject");
    // { code: 'ENOENT' }
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

// GHD: unit/get-file-hash-test.ts › get-file-hash › supports sha1 algorithm
#[test]
#[ignore = "ghd: missing: Corvene has no sha1 file hash, corvene_packs::sha256_file is sha256 only (getFileHash(path, 'sha1'), lib/get-file-hash.ts)"]
fn supports_sha1_algorithm() {
    let dir = mkdtemp();
    let file_path = dir.path().join("test-file.js");

    write_file(&file_path, "hello world");

    let hash = get_file_hash_sha1(&file_path).expect("getFileHash");

    // SHA-1 of "hello world"
    assert_eq!(hash, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
}
