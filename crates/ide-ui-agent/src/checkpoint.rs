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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use ide_core::explain::{Explain, Explained};
    use ide_git::{CommitOptions, Git};
    use ide_process::Supervisor;
    use ide_ui_common::window::WindowId;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSView;

    use super::*;
    use crate::AgentKey;
    use crate::work::Part;

    const WAIT: Duration = Duration::from_secs(30);

    /// An app whose only part the checkpoints reach is its git.
    struct GitOnly(Option<Git>);

    impl Host for GitOnly {
        fn state(&self) -> std::rc::Rc<crate::State> {
            unreachable!()
        }
        fn recent_output(&self, _: WindowId) -> Option<(String, String)> {
            unreachable!()
        }
        fn editor_selection(&self, _: WindowId) -> Option<(PathBuf, (u32, u32), String)> {
            unreachable!()
        }
        fn agent_diagnostics(&self, _: &Path) -> Arc<dyn ide_agent::Diagnostics> {
            unreachable!()
        }
        fn query_files(&self, _: WindowId, _: &str, _: usize, _: Box<dyn FnOnce(Vec<ide_project::Hit>) + Send>) {
            unreachable!()
        }
        fn git(&self) -> Option<Git> {
            self.0.clone()
        }
        fn unsaved_buffers(&self, _: &Path) -> Vec<(PathBuf, Vec<u8>)> {
            unreachable!()
        }
        fn take_back_buffers(&self, _: WindowId, _: Vec<(PathBuf, Vec<u8>)>) {
            unreachable!()
        }
        fn open_terminal(&self, _: WindowId, _: &Path) {
            unreachable!()
        }
        fn show_agent_settings(&self, _: MainThreadMarker) {
            unreachable!()
        }
        fn show_agents_settings(&self, _: MainThreadMarker) {
            unreachable!()
        }
        fn remember_claude_rules(&self, _: &Path) {
            unreachable!()
        }
        fn turn_ended(&self, _: WindowId, _: &Path) {
            unreachable!()
        }
        fn trust(&self, _: &Path) {
            unreachable!()
        }
        fn show_work(&self, _: WindowId, _: AgentKey, _: Part, _: &NSView) {
            unreachable!()
        }
        fn reread_file(&self, _: &Path) {
            unreachable!()
        }
    }

    fn store(folder: &Path, git: Option<Git>) -> Store {
        Store::new(folder.to_path_buf(), Box::leak(Box::new(GitOnly(git))))
    }

    /// git without the user's or the system's configuration, and a repository in `scratch` with `a.txt` committed.
    fn repository(scratch: &Path) -> (Git, PathBuf) {
        let home = scratch.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let git = Git::new(
            &Supervisor::new(),
            vec![
                ("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()),
                ("HOME".into(), home.to_string_lossy().into_owned()),
                ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
                ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ],
        );
        let root = scratch.join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git.init(&root).unwrap();
        let config = repo.git_dir().join("config");
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str("[user]\n\tname = Test\n\temail = test@example.invalid\n[commit]\n\tgpgsign = false\n");
        std::fs::write(&config, text).unwrap();
        std::fs::write(root.join("a.txt"), "before\n").unwrap();
        repo.stage_all().unwrap();
        repo.commit("first", CommitOptions::default()).unwrap();
        (git, repo.root().to_path_buf())
    }

    /// Takes a checkpoint of `store`'s folder and waits for it.
    fn create(store: &Store, buffers: Vec<(PathBuf, Vec<u8>)>) -> Option<Checkpoint> {
        let (tx, rx) = channel();
        store.create("thread-1".into(), "prompt".into(), buffers, move |c| {
            let _ = tx.send(c);
        });
        rx.recv_timeout(WAIT).unwrap()
    }

    /// Restores `checkpoint` of `store`'s folder and waits for the outcome.
    fn restore(store: &Store, checkpoint: Checkpoint) -> Result<Restore, RestoreError> {
        let (tx, rx) = channel();
        store.restore(checkpoint, RestoreOptions::default(), move |r| {
            let _ = tx.send(r);
        });
        rx.recv_timeout(WAIT).unwrap()
    }

    #[test]
    fn a_turn_taken_finished_and_restored_brings_the_folder_and_its_buffers_back() {
        let scratch = ide_core::testing::scratch("ui-agent-checkpoint");
        let (git, root) = repository(&scratch);
        let store = store(&root, Some(git));
        let created = create(&store, vec![(root.join("a.txt"), b"unsaved".to_vec())]).expect("a checkpoint");
        assert!(!created.finished);

        std::fs::write(root.join("a.txt"), "agent\n").unwrap();
        std::fs::write(root.join("agent.txt"), "made by the agent\n").unwrap();
        let (tx, rx) = channel();
        store.finish(created, move |c| {
            let _ = tx.send(c);
        });
        let finished = rx.recv_timeout(WAIT).unwrap();
        assert!(finished.finished);

        let restored = match restore(&store, finished) {
            Ok(Restore::Restored(restored)) => restored,
            other => panic!("not restored: {other:?}"),
        };
        assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "before\n");
        assert!(!root.join("agent.txt").exists());
        assert_eq!(restored.buffers, vec![(root.join("a.txt"), b"unsaved".to_vec())]);
    }

    #[test]
    fn without_git_no_checkpoint_is_taken_and_a_restore_says_why() {
        let scratch = ide_core::testing::scratch("ui-agent-checkpoint-no-git");
        let (git, root) = repository(&scratch);
        let checkpoint = create(&store(&root, Some(git)), Vec::new()).expect("a checkpoint");

        let missing = store(&root, None);
        assert!(create(&missing, Vec::new()).is_none());

        let (tx, rx) = channel();
        missing.finish(checkpoint.clone(), move |c| {
            let _ = tx.send(c);
        });
        assert!(
            rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "a failed finish answers nothing"
        );

        let why = match restore(&missing, checkpoint) {
            Err(error @ RestoreError::Git(ide_git::Error::GitMissing)) => error.explain(),
            other => panic!("not refused for the missing git: {other:?}"),
        };
        assert_eq!(why, ide_git::Error::GitMissing.explain());
        assert!(!why.is_empty());
    }

    #[test]
    fn a_restore_without_a_thread_says_why() {
        let error = RestoreError::NoThread(Explained("no thread".into()));
        assert_eq!(error.explain(), "no thread");
    }
}
