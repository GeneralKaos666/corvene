//! GitHub Desktop test port, lane `infra` (the reference port). Each module
//! is one GitHub Desktop test file; this root is merged into
//! `tests/ghd/main.rs` once the lanes are done (`tools/ghd-tests/README.md`).

#[path = "ghd/git_add.rs"]
mod git_add;
#[path = "ghd/git_description.rs"]
mod git_description;
#[path = "ghd/git_init.rs"]
mod git_init;
