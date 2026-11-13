//! Checkpoints of the folder around each turn (I6), taken on worker threads: one before the prompt goes out, finished
//! when the turn ends, restored from a user message's rewind menu.

use ide_core::LockIgnoringPoison;
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
        let mut open = self.open.lock_ignoring_poison();
        if open.is_none() {
            let env = ide_ui_common::window::environment(Duration::from_secs(10));
            let git = Git::new(Supervisor::global(), env);
            *open = Some(Checkpoints::open(&git, &self.folder).map_err(|e| e.to_string())?);
        }
        let checkpoints = open.as_ref().ok_or_else(String::new)?;
        f(checkpoints).map_err(|e| e.to_string())
    }

    /// Takes a checkpoint for `thread` with the unsaved `buffers` on a worker thread and hands it to `done` there;
    /// `None` when it failed.
    pub fn create(
        &self,
        thread: String,
        label: String,
        buffers: Vec<(PathBuf, Vec<u8>)>,
        done: impl FnOnce(Option<Checkpoint>) + Send + 'static,
    ) {
        let store = self.clone();
        let spawned = ide_ui_common::util::spawn_with("checkpoint", done, move |done| {
            let created = store.with(|c| c.create(&thread, &label, &buffers));
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
    pub fn finish(&self, checkpoint: Checkpoint, done: impl FnOnce(Checkpoint) + Send + 'static) {
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
    pub fn restore(
        &self,
        checkpoint: Checkpoint,
        options: RestoreOptions,
        done: impl FnOnce(Result<Restore, String>) + Send + 'static,
    ) {
        let store = self.clone();
        if let Err((done, why)) = ide_ui_common::util::spawn_with("checkpoint", done, move |done| {
            done(store.with(|c| {
                let mut restore = c.restore(&checkpoint, &options)?;
                if let Restore::Restored(done) = &mut restore {
                    for (path, _) in &mut done.buffers {
                        *path = c.root().join(&*path);
                    }
                }
                Ok(restore)
            }));
        }) {
            done(Err(why));
        }
    }
}
