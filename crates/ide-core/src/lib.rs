//! What the app knows and keeps without a window: its folders, settings in two layers, the shortcut table, each
//! project's window tabs, breakpoints and worker threads with a limit on what may wait; and what every crate shares:
//! locks that survive a panic, cancelling, threads for one piece of work, deadlines and the diagnostics hook.

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

mod cancel;
pub mod debugging;
pub mod diag;
pub mod explain;
pub mod keymap;
pub mod lane;
pub mod layout;
pub mod paths;
pub mod settings;
pub mod settings_sync;
mod store;
pub mod sync;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod thread;
pub mod timer;

pub use cancel::Cancel;
pub use sync::{IgnoringPoison, LockIgnoringPoison};
