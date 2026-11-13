//! A debug session as the panel shows it: where it stands, its threads and their call stacks, and what the adapter
//! says it can do.

use std::path::PathBuf;

/// What an adapter can do beyond the basics, from its answer to `initialize`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caps {
    /// It restarts on its own.
    pub restart: bool,
    /// It ends a program politely with `terminate`.
    pub terminate: bool,
    /// It takes conditional breakpoints.
    pub conditions: bool,
    /// It takes hit counts on breakpoints.
    pub hit_conditions: bool,
    /// It takes logpoints.
    pub log_points: bool,
    /// It evaluates expressions for hovers.
    pub hover_values: bool,
    /// The exception filters it offers: id, name and whether it is on unless the user says otherwise.
    pub exception_filters: Vec<(String, String, bool)>,
}

/// Whose the program is (Review C7): one the session started ends with it, one it attached to is only let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Started by the session.
    Launched,
    /// A program that already ran, attached to.
    Attached,
}

/// Where a session stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// The adapter starts and gets its configuration.
    Starting,
    /// The program runs.
    Running,
    /// Stopped in `thread`, for `reason` as the adapter says it ("breakpoint", "step", "exception", "pause").
    Paused {
        /// The thread that stopped.
        thread: i64,
        /// Why it stopped.
        reason: String,
    },
    /// Over, with the program's exit code if the adapter told it, or why it ended.
    Ended {
        /// The exit code.
        code: Option<i64>,
        /// Why it ended, if not normally.
        problem: Option<String>,
    },
}

/// A frame of a call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The adapter's id for it, valid while the session stays paused.
    pub id: i64,
    /// Its function's name.
    pub name: String,
    /// Its source file, if it has one.
    pub path: Option<PathBuf>,
    /// The line, from 0.
    pub line: usize,
    /// The column, from 0.
    pub column: usize,
}

/// A thread of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    /// The adapter's id for it.
    pub id: i64,
    /// Its name.
    pub name: String,
    /// Its call stack while the session is paused, innermost first; `None` until the adapter told it.
    pub frames: Option<Vec<Frame>>,
}

/// A debug session of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The app's id for it, which answers from the adapter carry.
    pub id: u64,
    /// Its name, as the configuration calls it.
    pub name: String,
    /// Whose the program is.
    pub origin: Origin,
    /// Where it stands.
    pub phase: Phase,
    /// What the adapter can do.
    pub caps: Caps,
    /// The program's threads.
    pub threads: Vec<Thread>,
    /// The start has gone unanswered for a while, as when macOS asks for the password to let a debugger take
    /// control of a program (Developer Tools Access).
    pub waiting: bool,
}

impl Session {
    /// A session that is starting.
    pub fn new(id: u64, name: impl Into<String>, origin: Origin) -> Session {
        Session {
            id,
            name: name.into(),
            origin,
            phase: Phase::Starting,
            caps: Caps::default(),
            threads: Vec::new(),
            waiting: false,
        }
    }

    /// Whether the program is stopped.
    pub fn is_paused(&self) -> bool {
        matches!(self.phase, Phase::Paused { .. })
    }

    /// Whether it ended.
    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Ended { .. })
    }

    /// The thread the session stopped in.
    pub fn paused_thread(&self) -> Option<i64> {
        match self.phase {
            Phase::Paused { thread, .. } => Some(thread),
            _ => None,
        }
    }

    /// The thread with `id`.
    pub fn thread(&self, id: i64) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    /// The thread with `id`, to change.
    pub fn thread_mut(&mut self, id: i64) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }

    /// Takes the threads the adapter listed, keeping the call stacks it already told of those that stay.
    pub fn set_threads(&mut self, threads: Vec<(i64, String)>) {
        let old = std::mem::take(&mut self.threads);
        self.threads = threads
            .into_iter()
            .map(|(id, name)| Thread {
                frames: old.iter().find(|t| t.id == id).and_then(|t| t.frames.clone()),
                id,
                name,
            })
            .collect();
    }

    /// Forgets every call stack, which the adapter's frame ids no longer reach once the program runs on.
    pub fn forget_frames(&mut self) {
        for thread in &mut self.threads {
            thread.frames = None;
        }
    }
}
