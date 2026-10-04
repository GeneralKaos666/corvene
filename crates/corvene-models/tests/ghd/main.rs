//! GitHub Desktop 3.6.6 unit tests ported to Corvene. Each module is one
//! GitHub Desktop test file (`app/test/unit/...`); `*_support` modules hold
//! helpers shared inside this crate. See `tools/ghd-tests/README.md`.

mod format_duration;
mod identifier_rules;
mod model_type_guards;
mod name_of;
mod remove_remote_prefix;
mod repository;
mod repository_matching;
