//! Screen-level view models, computed from `AppState` on the loop thread.
//! Records, not the core's types: UniFFI needs named fields and no
//! `PathBuf`/`usize`, and a screen wants exactly its slice.

pub mod changes;
pub mod diff;
pub mod repo_list;

pub use changes::*;
pub use diff::*;
pub use repo_list::*;
