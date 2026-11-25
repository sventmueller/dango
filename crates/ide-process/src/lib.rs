//! Every process the app starts goes through one supervisor: in a process group of its own, listed for the activity
//! view, and ended in order — a graceful step, then SIGTERM, then SIGKILL — when it is stopped or the app quits.
//! [`tool`] finds and runs the short-lived helper programs the installers and the discovery of servers need.
//!
//! For work orders (phase 13): [`gate`] runs a gate command as `Kind::Gate` with a deadline and an output cap and
//! confirms the end of its whole process group; [`sandbox`] builds the `sandbox-exec` profiles of the own agent's shell
//! tool and of gates; [`order_env`] narrows the login shell's environment to what an order may see.

mod agent_tree;
pub mod framing;
pub mod gate;
pub mod order_env;
pub mod pipes;
pub mod priority;
pub mod private_dir;
pub mod reaper;
pub mod sandbox;
pub mod shell_env;
pub mod signpost;
pub mod supervisor;
pub mod system;
pub mod tail;
pub mod tool;

pub use supervisor::{Ending, Event, Kind, ProcessId, ProcessInfo, Spawned, Subscription, Supervisor};
