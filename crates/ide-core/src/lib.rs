//! What the app knows and keeps without a window: its folders, settings in two layers, the shortcut table, each
//! project's window tabs and worker threads with a limit on what may wait.

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

pub mod keymap;
pub mod lane;
pub mod layout;
pub mod paths;
pub mod settings;
mod store;
