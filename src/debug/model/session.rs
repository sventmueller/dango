//! A debug session as the panel shows it: where it stands, its threads and their call stacks, and what the adapter
//! says it can do.

use std::path::PathBuf;

/// What an adapter can do beyond the basics, from its answer to `initialize`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caps {
    pub restart: bool,
    pub terminate: bool,
    pub conditions: bool,
    pub hit_conditions: bool,
    pub log_points: bool,
    pub hover_values: bool,
    /// The exception filters it offers: id, name and whether it is on unless the user says otherwise.
    pub exception_filters: Vec<(String, String, bool)>,
}

/// Whose the program is (Review C7): one the session started ends with it, one it attached to is only let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Launched,
    Attached,
}

/// Where a session stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// The adapter starts and gets its configuration.
    Starting,
    Running,
    /// Stopped in `thread`, for `reason` as the adapter says it ("breakpoint", "step", "exception", "pause").
    Paused {
        thread: i64,
        reason: String,
    },
    /// Over, with the program's exit code if the adapter told it, or why it ended.
    Ended {
        code: Option<i64>,
        problem: Option<String>,
    },
}

/// A frame of a call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The adapter's id for it, valid while the session stays paused.
    pub id: i64,
    pub name: String,
    pub path: Option<PathBuf>,
    /// The line, from 0.
    pub line: usize,
    /// The column, from 0.
    pub column: usize,
}

/// A thread of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: i64,
    pub name: String,
    /// Its call stack while the session is paused, innermost first; `None` until the adapter told it.
    pub frames: Option<Vec<Frame>>,
}

/// A debug session of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The app's id for it, which answers from the adapter carry.
    pub id: u64,
    pub name: String,
    pub origin: Origin,
    pub phase: Phase,
    pub caps: Caps,
    pub threads: Vec<Thread>,
    /// The start has gone unanswered for a while, as when macOS asks for the password to let a debugger take
    /// control of a program (Developer Tools Access).
    pub waiting: bool,
}

impl Session {
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

    pub fn is_paused(&self) -> bool {
        matches!(self.phase, Phase::Paused { .. })
    }

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

    pub fn thread(&self, id: i64) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

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
