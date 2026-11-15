//! What a session reports to the app.

use std::fmt;
use std::path::PathBuf;

use serde_json::json;

use super::Session;
use crate::connection::Responder;
use crate::error::Refusal;
use crate::types::{Breakpoint, Capabilities, End, Output, Program, Stop, TerminalRequest, ThreadId};

/// Something that happened in a session, delivered on a background thread with the session's id.
#[derive(Debug)]
pub enum Event {
    /// The adapter answered `initialize`; buttons and menu items follow its capabilities from now on.
    Initialized(Capabilities),
    /// The adapter changed its capabilities.
    Capabilities(Capabilities),
    /// The program was launched or attached to and the breakpoints are set; it runs.
    Running,
    /// The program stopped.
    Stopped(Stop),
    /// The program continues.
    Continued {
        /// The thread that continues, if the adapter says.
        thread: Option<ThreadId>,
        /// Whether all threads continue.
        all_threads: bool,
    },
    /// A thread started.
    ThreadStarted(ThreadId),
    /// A thread ended.
    ThreadExited(ThreadId),
    /// A line for the debug console.
    Output(Output),
    /// The adapter changed a breakpoint on its own, such as verifying it once a library loaded.
    Breakpoint(Breakpoint),
    /// The breakpoints of one file as the adapter set them, in the order they were asked for; sent when the
    /// session sets them during its start and for every `Session::set_breakpoints`.
    BreakpointsSet {
        /// The file.
        path: PathBuf,
        /// The lines asked for, from 1.
        lines: Vec<u32>,
        /// What the adapter made of each.
        breakpoints: Vec<Breakpoint>,
    },
    /// The program being debugged, as the adapter describes it.
    Program(Program),
    /// The program ended with this exit code; `Ended` follows.
    Exited(i64),
    /// The adapter wants the program run in a terminal tab; the app opens one and answers through the second field.
    /// The program then belongs to the terminal, like any shell (Review C7).
    RunInTerminal(TerminalRequest, TerminalAnswer),
    /// The adapter started a session of its own beneath this one, such as js-debug for each Node process; it reports
    /// with its own id through the same callback. The parent keeps it alive and stops it with itself.
    Child(Session),
    /// Values shown for the stopped program are out of date and need to be fetched again.
    Invalidated,
    /// Stopping was asked for and is under way.
    Stopping,
    /// The session is over.
    Ended(End),
}

/// Answers the adapter's `runInTerminal`; dropped unanswered, it tells the adapter the terminal could not start.
pub struct TerminalAnswer {
    pub(crate) responder: Responder,
}

impl TerminalAnswer {
    /// The program started; `process` is its number, `shell` the number of the shell that runs it, if known.
    pub fn started(self, process: Option<u32>, shell: Option<u32>) {
        let mut body = json!({});
        if let Some(pid) = process {
            body["processId"] = json!(pid);
        }
        if let Some(pid) = shell {
            body["shellProcessId"] = json!(pid);
        }
        self.responder.ok(body);
    }

    /// The terminal could not run it, for the reason the adapter is told.
    pub fn failed(self, why: Refusal) {
        self.responder.fail(why);
    }
}

impl fmt::Debug for TerminalAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TerminalAnswer")
    }
}
