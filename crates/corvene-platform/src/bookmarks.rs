//! File bookmarks (macOS `NSURL` bookmark data): a reference to a folder
//! that still finds it after it was moved or renamed on its volume, for
//! Corvene's `294-follow-moved-repositories`. Other platforms have none
//! ([`bookmark`] returns `None`).
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::{Path, PathBuf};

/// Bookmark data for `path` (an existing file or folder).
pub fn bookmark(path: &Path) -> Option<Vec<u8>> {
    imp::bookmark(path)
}

/// Where the item `data` (from [`bookmark`]) is now, if it still exists.
pub fn resolve_bookmark(data: &[u8]) -> Option<PathBuf> {
    imp::resolve_bookmark(data)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{CStr, CString, c_char};
    use std::path::{Path, PathBuf};

    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    /// `NSURLBookmarkResolutionWithoutUI | NSURLBookmarkResolutionWithoutMounting`
    const RESOLVE_QUIETLY: u64 = (1 << 8) | (1 << 9);

    pub fn bookmark(path: &Path) -> Option<Vec<u8>> {
        let c_path = CString::new(path.to_str()?).ok()?;
        let nil: *mut Object = std::ptr::null_mut();
        // SAFETY: Foundation message sends; everything returned is
        // autoreleased into the pool drained here, the bytes are copied out
        // before it is.
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let ns_path: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: c_path.as_ptr()];
            let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_path];
            let mut error: *mut Object = std::ptr::null_mut();
            let data: *mut Object = if url.is_null() {
                nil
            } else {
                msg_send![url, bookmarkDataWithOptions: 0u64
                    includingResourceValuesForKeys: nil
                    relativeToURL: nil
                    error: &mut error]
            };
            let mut bytes = None;
            if !data.is_null() {
                let len: usize = msg_send![data, length];
                let ptr: *const u8 = msg_send![data, bytes];
                if !ptr.is_null() && len > 0 {
                    bytes = Some(std::slice::from_raw_parts(ptr, len).to_vec());
                }
            }
            let _: () = msg_send![pool, drain];
            bytes
        }
    }

    pub fn resolve_bookmark(bytes: &[u8]) -> Option<PathBuf> {
        if bytes.is_empty() {
            return None;
        }
        let nil: *mut Object = std::ptr::null_mut();
        // SAFETY: as in `bookmark`; `NSData dataWithBytes:length:` copies the
        // slice, and the path string is copied before the pool is drained.
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let data: *mut Object = msg_send![class!(NSData),
                dataWithBytes: bytes.as_ptr()
                length: bytes.len()];
            let mut stale: BOOL = NO;
            let mut error: *mut Object = std::ptr::null_mut();
            let url: *mut Object = msg_send![class!(NSURL),
                URLByResolvingBookmarkData: data
                options: RESOLVE_QUIETLY
                relativeToURL: nil
                bookmarkDataIsStale: &mut stale
                error: &mut error];
            let mut path = None;
            if !url.is_null() {
                let ns_path: *mut Object = msg_send![url, path];
                if !ns_path.is_null() {
                    let utf8: *const c_char = msg_send![ns_path, UTF8String];
                    if !utf8.is_null() {
                        path = Some(PathBuf::from(
                            CStr::from_ptr(utf8).to_string_lossy().into_owned(),
                        ));
                    }
                }
            }
            let _: () = msg_send![pool, drain];
            path
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::path::{Path, PathBuf};

    pub fn bookmark(_: &Path) -> Option<Vec<u8>> {
        None
    }

    pub fn resolve_bookmark(_: &[u8]) -> Option<PathBuf> {
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn a_bookmark_follows_a_moved_folder() {
        let dir = tempfile::tempdir().unwrap();
        let before = dir.path().join("before");
        std::fs::create_dir(&before).unwrap();
        let data = bookmark(&before).unwrap();
        let after = dir.path().join("after");
        std::fs::rename(&before, &after).unwrap();
        let resolved = resolve_bookmark(&data).unwrap();
        assert_eq!(
            std::fs::canonicalize(resolved).unwrap(),
            std::fs::canonicalize(&after).unwrap()
        );
    }
}
