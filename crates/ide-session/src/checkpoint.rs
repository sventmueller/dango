//! The checkpoint before each prompt and its end after the turn (I6, E34), taken on worker threads; the result comes
//! back through the driver's inbox with the turn's id.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use ide_git::{Checkpoint, CheckpointThread, Checkpoints, Git};

use crate::id::TurnId;
use crate::inbox::{Inbound, Inbox};

/// The checkpoints of one session's folder under one checkpoint thread, opened on first use.
#[derive(Clone)]
pub(crate) struct Checkpointer {
    git: Git,
    folder: PathBuf,
    thread: String,
    open: Arc<Mutex<Option<Checkpoints>>>,
}

impl Checkpointer {
    pub(crate) fn new(git: Git, folder: PathBuf, thread: String) -> Checkpointer {
        Checkpointer {
            git,
            folder,
            thread,
            open: Arc::new(Mutex::new(None)),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&Checkpoints, &CheckpointThread) -> ide_git::Result<R>) -> Result<R, String> {
        let thread = CheckpointThread::new(self.thread.clone()).map_err(|e| e.to_string())?;
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        let checkpoints = match open.take() {
            Some(checkpoints) => checkpoints,
            None => Checkpoints::open(&self.git, &self.folder).map_err(|e| e.to_string())?,
        };
        f(open.insert(checkpoints), &thread).map_err(|e| e.to_string())
    }

    /// Takes the checkpoint before the prompt of `turn`, with the unsaved `buffers`.
    pub(crate) fn create(&self, turn: TurnId, label: String, buffers: Vec<(PathBuf, Vec<u8>)>, inbox: Arc<Inbox>) {
        let this = self.clone();
        let reporting = inbox.clone();
        let spawned = std::thread::Builder::new()
            .name("session-checkpoint".into())
            .spawn(move || {
                let result = this.with(|c, thread| c.create(thread, &label, &buffers).map(|made| made.checkpoint));
                inbox.push(Inbound::Checkpointed { turn, result });
            });
        if let Err(error) = spawned {
            reporting.push(Inbound::Checkpointed {
                turn,
                result: Err(error.to_string()),
            });
        }
    }

    /// Records the state the turn `turn`, which `checkpoint` started, ended in.
    pub(crate) fn finish(&self, turn: TurnId, checkpoint: Checkpoint, inbox: Arc<Inbox>) {
        let this = self.clone();
        let reporting = inbox.clone();
        let spawned = std::thread::Builder::new()
            .name("session-checkpoint".into())
            .spawn(move || {
                if let Err(message) = this.with(|c, _| c.finish(&checkpoint)) {
                    inbox.push(Inbound::FinishFailed { turn, message });
                }
            });
        if let Err(error) = spawned {
            reporting.push(Inbound::FinishFailed {
                turn,
                message: error.to_string(),
            });
        }
    }
}
