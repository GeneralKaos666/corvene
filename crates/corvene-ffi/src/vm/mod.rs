//! Screen-level view models, computed from `AppState` on the loop thread.
//! Records, not the core's types: UniFFI needs named fields and no
//! `PathBuf`/`usize`, and a screen wants exactly its slice.

pub mod branches;
pub mod changes;
pub mod diff;
pub mod history;
pub mod popup;
pub mod repo_list;
pub mod settings;

pub use branches::*;
pub use changes::*;
pub use diff::*;
pub use history::*;
pub use popup::*;
pub use repo_list::*;
pub use settings::*;
