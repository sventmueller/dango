//! Short-lived helper processes of an installation, such as `npm install` or `ditto`, run through the supervisor with
//! the login shell's environment and a time limit.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use ide_process::{Ending, Event, Kind, ProcessId, Supervisor};

/// The most of a helper's output that is kept.
const KEPT: u64 = 256 * 1024;

/// How many endings are remembered; a helper is looked up right after it ends.
const REMEMBERED: usize = 64;

type Board = (Mutex<VecDeque<(ProcessId, Ending)>>, Condvar);

/// What a helper printed and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Output {
    pub(crate) ending: Option<Ending>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Output {
    pub(crate) fn succeeded(&self) -> bool {
        self.ending == Some(Ending::Code(0))
    }
}

/// Runs helpers; it hears the supervisor's endings once rather than subscribing for each helper, since a
/// subscription cannot be taken back.
#[derive(Clone)]
pub(crate) struct Runner {
    supervisor: Supervisor,
    env: Vec<(String, String)>,
    board: Arc<Board>,
}

fn read_all(reader: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut reader) = reader {
            let _ = (&mut reader).take(KEPT).read_to_end(&mut bytes);
            let _ = std::io::copy(&mut reader, &mut std::io::sink());
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

impl Runner {
    pub(crate) fn new(supervisor: &Supervisor, env: Vec<(String, String)>) -> Runner {
        let board: Arc<Board> = Arc::default();
        let heard = Arc::downgrade(&board);
        supervisor.subscribe(move |event| {
            let (Event::Ended { id, ending }, Some(board)) = (event, heard.upgrade()) else {
                return;
            };
            let (lock, changed) = &*board;
            let mut endings = lock.lock().unwrap_or_else(|e| e.into_inner());
            if endings.len() == REMEMBERED {
                endings.pop_front();
            }
            endings.push_back((*id, *ending));
            changed.notify_all();
        });
        Runner {
            supervisor: supervisor.clone(),
            env,
            board,
        }
    }

    fn wait(&self, id: ProcessId, deadline: Instant) -> Option<Ending> {
        let (lock, changed) = &*self.board;
        let mut endings = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some((_, ending)) = endings.iter().find(|(i, _)| *i == id) {
                return Some(*ending);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            endings = changed
                .wait_timeout(endings, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Runs `program` with `args` in `dir`, with `extra` added to the environment, and waits at most `timeout`.
    pub(crate) fn run(
        &self,
        program: &Path,
        args: &[&str],
        dir: &Path,
        extra: &[(String, String)],
        timeout: Duration,
    ) -> std::io::Result<Output> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(self.env.iter().chain(extra).map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let label = program
            .file_name()
            .map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into_owned());
        let spawned = self.supervisor.spawn(command, Kind::Tool, label)?;
        let stdout = read_all(spawned.stdout);
        let stderr = read_all(spawned.stderr);
        let Some(ending) = self.wait(spawned.id, Instant::now() + timeout) else {
            self.supervisor.stop(spawned.id, Duration::from_millis(500));
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{} took too long", program.display()),
            ));
        };
        Ok(Output {
            ending: Some(ending),
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
        })
    }

    /// The first executable called `name` on the login shell's PATH.
    pub(crate) fn which(&self, name: &str) -> Option<PathBuf> {
        which(name, var(&self.env, "PATH"))
    }
}

/// The first executable called `name` in the folders of `path`, a PATH-style list.
pub(crate) fn which(name: &str, path: Option<&str>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path?
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The value of `key` in `env`.
pub(crate) fn var<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
    env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}
