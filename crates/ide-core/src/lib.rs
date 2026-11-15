//! What the app knows and keeps without a window: its folders, settings in two layers, the shortcut table, each
//! project's window tabs with the rules of their panes and the parts features keep beside them, and worker threads
//! with a limit on what may wait; and what every crate shares: locks that survive a panic, cancelling, threads for one
//! piece of work, deadlines, the diagnostics hook, files replaced as a whole and answers kept while a file keeps its
//! stamp.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod atomic_write;
mod cancel;
pub mod diag;
pub mod explain;
pub mod keymap;
pub mod lane;
pub mod layout;
pub mod panes;
pub mod paths;
pub mod settings;
pub mod settings_sync;
pub mod stamp;
mod store;
pub mod sync;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod thread;
pub mod timer;

pub use cancel::Cancel;
pub use sync::{IgnoringPoison, LockIgnoringPoison};
