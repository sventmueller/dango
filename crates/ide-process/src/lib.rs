//! Every process the app starts goes through one supervisor: in a process group of its own, listed for the activity
//! view, and ended in order — a graceful step, then SIGTERM, then SIGKILL — when it is stopped or the app quits.
//! [`tool`] finds and runs the short-lived helper programs the installers and the discovery of servers need.

#![warn(missing_docs)]

pub mod framing;
pub mod pipes;
pub mod priority;
pub mod reaper;
pub mod shell_env;
pub mod signpost;
pub mod supervisor;
pub mod system;
pub mod tail;
pub mod tool;

pub use supervisor::{Ending, Event, Kind, ProcessId, ProcessInfo, Spawned, Subscription, Supervisor};
