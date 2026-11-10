//! Checkpoints of the folder around each turn (I6), taken on worker threads: one before the prompt goes out, finished
//! when the turn ends, restored from a user message's rewind menu.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_git::{Checkpoint, Checkpoints, Git, Restore, RestoreOptions};
use ide_process::Supervisor;

/// The checkpoints of one folder, opened on first use.
#[derive(Clone)]
pub struct Store {
    folder: PathBuf,
    open: Arc<Mutex<Option<Checkpoints>>>,
}

impl Store {
    pub fn new(folder: PathBuf) -> Store {
        Store {
            folder,
            open: Arc::new(Mutex::new(None)),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&Checkpoints) -> ide_git::Result<R>) -> Result<R, String> {
        let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
        if open.is_none() {
            let env = crate::environment::wait(Duration::from_secs(10))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            let git = Git::new(Supervisor::global(), env);
            *open = Some(Checkpoints::open(&git, &self.folder).map_err(|e| e.to_string())?);
        }
        let checkpoints = open.as_ref().ok_or_else(String::new)?;
        f(checkpoints).map_err(|e| e.to_string())
    }

    /// Takes a checkpoint for `thread` on a worker thread and hands it to `done` there; `None` when it failed.
    pub fn create(&self, thread: String, label: String, done: impl FnOnce(Option<Checkpoint>) + Send + 'static) {
        let store = self.clone();
        let spawned = std::thread::Builder::new().name("checkpoint".into()).spawn(move || {
            let created = store.with(|c| c.create(&thread, &label, &[]));
            match created {
                Ok(created) => done(Some(created.checkpoint)),
                Err(error) => {
                    crate::diag::report("checkpoint before a turn", error);
                    done(None);
                }
            }
        });
        if spawned.is_err() {
            crate::diag::report("checkpoint before a turn", "no thread");
        }
    }

    /// Records the end of the turn `checkpoint` started, on a worker thread.
    pub fn finish(&self, checkpoint: Checkpoint, done: impl FnOnce(Checkpoint) + Send + 'static) {
        let store = self.clone();
        let _ = std::thread::Builder::new().name("checkpoint".into()).spawn(move || {
            match store.with(|c| c.finish(&checkpoint)) {
                Ok(finished) => done(finished),
                Err(error) => crate::diag::report("checkpoint after a turn", error),
            }
        });
    }

    /// Brings `checkpoint` back on a worker thread and hands the outcome to `done` there.
    pub fn restore(
        &self,
        checkpoint: Checkpoint,
        options: RestoreOptions,
        done: impl FnOnce(Result<Restore, String>) + Send + 'static,
    ) {
        let store = self.clone();
        let _ = std::thread::Builder::new()
            .name("checkpoint".into())
            .spawn(move || done(store.with(|c| c.restore(&checkpoint, &options))));
    }
}
