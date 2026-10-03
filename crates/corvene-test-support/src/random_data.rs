//! Port of `app/test/helpers/random-data.ts`.

/// The length GitHub Desktop's `generateString()` uses without an argument.
pub const DEFAULT_STRING_LENGTH: usize = 32;

/// GitHub Desktop's `generateString(length)`: `length` pseudo-random
/// lowercase hexadecimal characters (pass [`DEFAULT_STRING_LENGTH`] for
/// `generateString()`).
pub fn generate_string(length: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    (0..length)
        .map(|_| char::from(HEX[fastrand::usize(..HEX.len())]))
        .collect()
}
