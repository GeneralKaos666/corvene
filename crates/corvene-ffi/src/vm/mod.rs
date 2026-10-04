//! Screen-level view models, computed from `AppState` on the loop thread.
//! Records, not the core's types: UniFFI needs named fields and no
//! `PathBuf`/`usize`, and a screen wants exactly its slice.

pub mod branches;
pub mod changes;
pub mod diff;
pub mod flags;
pub mod history;
pub mod mco;
pub mod popup;
pub mod pull_requests;
pub mod repo_list;
pub mod repo_settings;
pub mod session;
pub mod settings;

pub use branches::*;
pub use changes::*;
pub use diff::*;
pub use flags::*;
pub use history::*;
pub use mco::*;
pub use popup::*;
pub use pull_requests::*;
pub use repo_list::*;
pub use repo_settings::*;
pub use session::*;
pub use settings::*;
