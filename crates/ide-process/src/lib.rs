//! Every process the app starts goes through one supervisor: in a process group of its own, listed for the activity
//! view, and ended in order — a graceful step, then SIGTERM, then SIGKILL — when it is stopped or the app quits.

#![warn(missing_docs, unreachable_pub)]

pub mod framing;
pub mod priority;
pub mod shell_env;
pub mod supervisor;
pub mod tail;

pub use supervisor::{Ending, Event, Kind, Origin, ProcessId, ProcessInfo, Spawned, Supervisor};
