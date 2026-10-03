//! GitHub Desktop 3.6.6 unit tests ported to Corvene. Each module is one
//! GitHub Desktop test file (`app/test/unit/...`); `*_support` modules hold
//! helpers shared inside this crate. See `tools/ghd-tests/README.md`.

mod api;
mod api_error_handling;
mod api_support;
mod email;
mod endpoint_capabilities;
mod enterprise_validate_url;
mod http;
