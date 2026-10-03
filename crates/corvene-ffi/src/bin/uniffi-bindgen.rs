//! `cargo run -p corvene-ffi --bin uniffi-bindgen -- generate --library
//! <libcorvene_ffi.so> --language kotlin --out-dir <dir> --config
//! crates/corvene-ffi/uniffi.toml`

fn main() {
    uniffi::uniffi_bindgen_main()
}
