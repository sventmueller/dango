//! Debugging through the Debug Adapter Protocol (B1 in docs/phase-11.md, E93): sessions with their adapters, the
//! adapters the app knows and installs, launch configurations, and who owns which process (Review C7).
//!
//! A [`Session`] speaks to one adapter on threads of its own, so no call blocks and no async runtime runs beside
//! AppKit's run loop: requests answer through a [`Reply`] on a background thread, and [`Event`]s arrive through the
//! callback given at the start, each with the session's [`SessionId`]. Lines and columns count from 1, columns in
//! UTF-16 code units.
//!
//! The usual way in is [`start`]: it checks trust, builds what the configuration asks Cargo for, finds the adapter
//! with [`Adapters`] and starts the session. Configurations come from `.vscode/launch.json` ([`launch_configs`]),
//! from the file in the editor ([`current_program`]) or from a process to attach to ([`processes`],
//! [`LaunchConfig::attach`]).
//!
//! Who owns what: the adapter always belongs to the app and runs under `ide_process::Supervisor`. A launched program
//! belongs to its session, and [`Session::stop`] ends it: `terminate`, after a grace period `disconnect`, last the
//! supervisor ends the adapter's process group, and a program that outlived it goes with its own group. An attached
//! program is never signalled: stopping detaches with `disconnect` and `terminateDebuggee: false`. A program the
//! adapter runs through `runInTerminal` belongs to that terminal tab. [`stop_all`] does this for every session when
//! the app quits, and the supervisor's own shutdown does it too through the graceful step each session registers.
//!
//! Nothing a project configures runs before the user trusted the folder: [`start`], [`launch_configs`] and
//! [`current_program`] take that decision as an argument and refuse without it. With the feature `testing`,
//! `fake::Script` is an adapter without a process, for tests and the self-test.
//!
//! [`model`] is what the app's debugger panel decides without a window (E98): its sessions, the selected frame,
//! breakpoints, the inspector, consoles and which commands can run, with the effects the controller carries out.

#![warn(missing_docs, unreachable_pub)]

mod adapter;
mod connection;
mod convert;
mod current;
mod discover;
mod error;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
mod filters;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod helper;
mod install;
mod jsonc;
mod launch;
pub mod model;
mod ownership;
mod prepare;
mod processes;
pub mod protocol;
pub mod replay;
mod session;
#[cfg(test)]
mod stub_tests;
mod types;

pub use adapter::{Adapter, AdapterKind, free_port};
pub use current::{CurrentError, current_program};
pub use discover::{Adapters, Install, Missing};
pub use error::Error;
pub use filters::{Languages, RUST_PANIC, offered as offered_filters, rust_panic};
pub use install::{JS_DEBUG, Pinned};
pub use launch::{ConfigError, Entry, LaunchConfig, LaunchError, Variables, launch_configs, parse_launch_json};
pub use prepare::{StartError, prepare, start};
pub use processes::{Process, processes};
pub use session::{Event, Session, Setup, TerminalAnswer, stop_all, wait};
pub use types::{
    Breakpoint, Capabilities, End, EndReason, EvaluateContext, Evaluation, ExceptionFilter, FrameId, Output,
    OutputCategory, Program, Reply, Request, Scope, SessionId, Source, SourceBreakpoint, StackFrame, StartMethod,
    State, Stop, StopReason, TerminalRequest, Thread, ThreadId, Variable, VariablesRef,
};
