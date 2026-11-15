//! Checkpoints of the folder around each turn (I6), taken on worker threads: one before the prompt goes out, finished
//! when the turn ends, restored from a user message's rewind menu.

use ide_core::LockIgnoringPoison;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ide_git::{Checkpoint, CheckpointThread, Checkpoints, Restore, RestoreOptions};

use crate::Host;

/// The checkpoints of one folder, opened on first use with the app's git.
#[derive(Clone)]
pub(crate) struct Store {
    folder: PathBuf,
    host: &'static (dyn Host + Sync),
    open: Arc<Mutex<Option<Checkpoints>>>,
}

impl Store {
    pub(crate) fn new(folder: PathBuf, host: &'static (dyn Host + Sync)) -> Store {
        Store {
            folder,
            host,
            open: Arc::new(Mutex::new(None)),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&Checkpoints) -> ide_git::Result<R>) -> ide_git::Result<R> {
        let mut open = self.open.lock_ignoring_poison();
        let checkpoints = match open.take() {
            Some(checkpoints) => checkpoints,
            None => {
                let git = self.host.git().ok_or(ide_git::Error::GitMissing)?;
                Checkpoints::open(&git, &self.folder)?
            }
        };
        f(open.insert(checkpoints))
    }

    /// Takes a checkpoint for `thread` with the unsaved `buffers` on a worker thread and hands it to `done` there;
    /// `None` when it failed.
    pub(crate) fn create(
        &self,
        thread: String,
        label: String,
        buffers: Vec<(PathBuf, Vec<u8>)>,
        done: impl FnOnce(Option<Checkpoint>) + Send + 'static,
    ) {
        let store = self.clone();
        let spawned = ide_ui_common::util::spawn_with("checkpoint", done, move |done| {
            let created = store.with(|c| c.create(&CheckpointThread::new(thread)?, &label, &buffers));
            match created {
                Ok(created) => done(Some(created.checkpoint)),
                Err(error) => {
                    ide_ui_common::diag::report("checkpoint before a turn", error);
                    done(None);
                }
            }
        });
        if let Err((done, why)) = spawned {
            ide_ui_common::diag::report("checkpoint before a turn", why);
            done(None);
        }
    }

    /// Records the end of the turn `checkpoint` started, on a worker thread.
    pub(crate) fn finish(&self, checkpoint: Checkpoint, done: impl FnOnce(Checkpoint) + Send + 'static) {
        let store = self.clone();
        if let Err(why) =
            ide_ui_common::util::spawn("checkpoint", move || match store.with(|c| c.finish(&checkpoint)) {
                Ok(finished) => done(finished),
                Err(error) => ide_ui_common::diag::report("checkpoint after a turn", error),
            })
        {
            ide_ui_common::diag::report("checkpoint after a turn", why);
        }
    }

    /// Brings `checkpoint` back on a worker thread and hands the outcome to `done` there, the buffers by full path.
    pub(crate) fn restore(
        &self,
        checkpoint: Checkpoint,
        options: RestoreOptions,
        done: impl FnOnce(Result<Restore, RestoreError>) + Send + 'static,
    ) {
        let store = self.clone();
        if let Err((done, why)) = ide_ui_common::util::spawn_with("checkpoint", done, move |done| {
            let restored = store.with(|c| {
                let mut restore = c.restore(&checkpoint, &options)?;
                if let Restore::Restored(done) = &mut restore {
                    for (path, _) in &mut done.buffers {
                        *path = c.root().join(&*path);
                    }
                }
                Ok(restore)
            });
            done(restored.map_err(RestoreError::Git));
        }) {
            done(Err(RestoreError::NoThread(why)));
        }
    }
}

/// Why a checkpoint did not come back.
#[derive(Debug)]
pub(crate) enum RestoreError {
    /// git refused.
    Git(ide_git::Error),
    /// No thread could do it.
    NoThread(ide_core::explain::Explained),
}

impl ide_core::explain::Explain for RestoreError {
    fn explain(&self) -> String {
        match self {
            RestoreError::Git(error) => ide_core::explain::Explain::explain(error),
            RestoreError::NoThread(why) => why.0.clone(),
        }
    }
}
