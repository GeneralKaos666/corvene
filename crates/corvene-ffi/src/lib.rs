//! The engine of the Android app: `corvene_core::Dispatcher` on a
//! `LocalHost` run loop, behind a UniFFI interface the Kotlin side drives.
//!
//! Kotlin creates one [`Corvene`] per process, dispatches actions (fire and
//! forget; the result shows up as state) and queries screen-level view
//! models after every `HostEvents::state_changed`. Everything the host does
//! for the engine (open a URL, pick a folder, the clipboard, toasts) is a
//! method of the `HostEvents` trait Kotlin implements.

uniffi::setup_scaffolding!();

mod api;
mod bridge;
mod runtime;
mod vm;

pub use api::*;
pub use vm::*;
