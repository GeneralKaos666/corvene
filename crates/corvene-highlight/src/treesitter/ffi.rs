//! The C-ABI grammar table `corvene-grammars` exports (`corvene_grammars_v1`).
//! Mirrored here because the default build does not link that crate; the
//! layouts are checked against it in tests.

use std::ffi::c_char;

/// Must equal `corvene_grammars::ABI`.
pub const ABI: u32 = 1;

/// The exported symbol returning a `*const Table`.
pub const ENTRY_SYMBOL: &[u8] = b"corvene_grammars_v1\0";

#[repr(C)]
pub struct Grammar {
    pub name: *const c_char,
    pub language: unsafe extern "C" fn() -> *const (),
    pub highlights: *const c_char,
    pub injections: *const c_char,
    pub locals: *const c_char,
    pub extensions: *const c_char,
    pub filenames: *const c_char,
    pub first_line: *const c_char,
    pub aliases: *const c_char,
    pub injects: *const c_char,
}

#[repr(C)]
pub struct Table {
    pub abi: u32,
    pub version: *const c_char,
    pub len: usize,
    pub grammars: *const Grammar,
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn layouts_match_corvene_grammars() {
        assert_eq!(super::ABI, corvene_grammars::ABI);
        assert_eq!(
            size_of::<super::Table>(),
            size_of::<corvene_grammars::Table>()
        );
        assert_eq!(
            align_of::<super::Table>(),
            align_of::<corvene_grammars::Table>()
        );
        assert_eq!(
            offset_of!(super::Table, grammars),
            offset_of!(corvene_grammars::Table, grammars)
        );
        assert_eq!(
            size_of::<super::Grammar>(),
            size_of::<corvene_grammars::Grammar>()
        );
        assert_eq!(
            offset_of!(super::Grammar, injects),
            offset_of!(corvene_grammars::Grammar, injects)
        );
        assert_eq!(
            offset_of!(super::Grammar, first_line),
            offset_of!(corvene_grammars::Grammar, first_line)
        );
    }
}
