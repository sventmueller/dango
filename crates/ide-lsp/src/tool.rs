//! Short-lived helper processes, such as `go env` or an installer, run through the supervisor with a time limit.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use ide_process::{Ending, Event, Kind, ProcessId, Supervisor};

/// How many endings are remembered; a process is looked up right after it ends.
const REMEMBERED: usize = 256;

type Board = (Mutex<VecDeque<(ProcessId, Ending)>>, Condvar);

/// The endings of recent processes, heard from the supervisor once instead of subscribing per process.
#[derive(Clone)]
pub(crate) struct Exits {
    board: Arc<Board>,
}

impl Exits {
    pub(crate) fn watch(supervisor: &Supervisor) -> Exits {
        let exits = Exits { board: Arc::default() };
        let board = Arc::downgrade(&exits.board);
        supervisor.subscribe(move |event| {
            let (Event::Ended { id, ending }, Some(board)) = (event, board.upgrade()) else {
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
        exits
    }

    /// How `id` ended, waiting up to `timeout` for it.
    pub(crate) fn wait(&self, id: ProcessId, timeout: Duration) -> Option<Ending> {
        let deadline = Instant::now() + timeout;
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
}

/// A request to stop a long task, such as an installation, shared between the asker and the task.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A request that has not been made yet.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Asks the task to stop as soon as it can.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether stopping was asked for.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a helper process printed and how it ended.
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

/// Why a helper process gave no output.
#[derive(Debug)]
pub(crate) enum RunError {
    Start(std::io::Error),
    Timeout,
    Cancelled,
}

/// What a helper process is started with.
pub(crate) struct Job<'a> {
    pub(crate) supervisor: &'a Supervisor,
    pub(crate) exits: &'a Exits,
    pub(crate) env: &'a [(String, String)],
    pub(crate) timeout: Duration,
    pub(crate) cancel: Option<&'a Cancel>,
}

/// The most a helper's stdout or stderr keeps.
const KEPT: usize = 1024 * 1024;

impl Job<'_> {
    /// Runs `program` with `args` in `dir`; `line` hears each line of standard error as it comes.
    pub(crate) fn run(
        &self,
        program: &Path,
        args: &[&str],
        dir: &Path,
        line: &mut dyn FnMut(&str),
    ) -> Result<Output, RunError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let label = program
            .file_name()
            .map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into());
        let spawned = self
            .supervisor
            .spawn(command, Kind::Tool, label)
            .map_err(RunError::Start)?;
        let (stdout, stderr) = (spawned.stdout, spawned.stderr);
        let out = std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut stdout) = stdout {
                let _ = (&mut stdout).take(KEPT as u64).read_to_end(&mut text);
                let _ = std::io::copy(&mut stdout, &mut std::io::sink());
            }
            String::from_utf8_lossy(&text).into_owned()
        });
        let (lines, heard) = channel();
        std::thread::spawn(move || {
            if let Some(stderr) = stderr {
                for l in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if lines.send(l).is_err() {
                        break;
                    }
                }
            }
        });
        let deadline = Instant::now() + self.timeout;
        let mut stderr = String::new();
        loop {
            match heard.recv_timeout(Duration::from_millis(50)) {
                Ok(l) => {
                    line(&l);
                    if stderr.len() < KEPT {
                        stderr.push_str(&l);
                        stderr.push('\n');
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self.cancel.is_some_and(Cancel::is_cancelled) {
                self.supervisor.stop(spawned.id, Duration::from_millis(500));
                return Err(RunError::Cancelled);
            }
            if Instant::now() >= deadline {
                self.supervisor.stop(spawned.id, Duration::from_millis(500));
                return Err(RunError::Timeout);
            }
        }
        let stdout = out.join().unwrap_or_default();
        let ending = self.exits.wait(
            spawned.id,
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_secs(1)),
        );
        Ok(Output { ending, stdout, stderr })
    }
}

/// The first executable called `name` on `path`, a PATH-style list of folders.
pub(crate) fn which(name: &str, path: Option<&str>) -> Option<PathBuf> {
    path?
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| is_executable(candidate))
}

/// Whether `path` is a file someone may execute.
pub(crate) fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The value of `name` in `env`.
pub(crate) fn var<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Vec<(String, String)> {
        vec![("PATH".into(), "/usr/bin:/bin".into())]
    }

    #[test]
    fn a_helper_reports_its_output_lines_and_exit_code() {
        let supervisor = Supervisor::new();
        let exits = Exits::watch(&supervisor);
        let env = env();
        let job = Job {
            supervisor: &supervisor,
            exits: &exits,
            env: &env,
            timeout: Duration::from_secs(10),
            cancel: None,
        };
        let mut heard = vec![];
        let output = job
            .run(
                Path::new("/bin/sh"),
                &["-c", "echo out; echo e1 >&2; echo e2 >&2; exit 4"],
                Path::new("/"),
                &mut |l| {
                    heard.push(l.to_string());
                },
            )
            .unwrap();
        assert_eq!(output.stdout, "out\n");
        assert_eq!(heard, ["e1", "e2"]);
        assert_eq!(output.ending, Some(Ending::Code(4)));
        assert!(!output.succeeded());
    }

    #[test]
    fn a_helper_is_stopped_when_it_takes_too_long_or_is_cancelled() {
        let supervisor = Supervisor::new();
        let exits = Exits::watch(&supervisor);
        let env = env();
        let slow = Job {
            supervisor: &supervisor,
            exits: &exits,
            env: &env,
            timeout: Duration::from_millis(200),
            cancel: None,
        };
        let result = slow.run(Path::new("/bin/sleep"), &["30"], Path::new("/"), &mut |_| {});
        assert!(matches!(result, Err(RunError::Timeout)));
        let cancel = Cancel::new();
        cancel.cancel();
        let cancelled = Job {
            cancel: Some(&cancel),
            timeout: Duration::from_secs(30),
            ..slow
        };
        let result = cancelled.run(Path::new("/bin/sleep"), &["30"], Path::new("/"), &mut |_| {});
        assert!(matches!(result, Err(RunError::Cancelled)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !supervisor.running().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(supervisor.running().is_empty());
    }

    #[test]
    fn executables_are_found_on_the_given_path_only() {
        assert_eq!(which("sh", Some("/nonexistent:/bin")), Some(PathBuf::from("/bin/sh")));
        assert_eq!(which("sh", Some("/nonexistent")), None);
        assert_eq!(which("sh", None), None);
        assert_eq!(var(&env(), "PATH"), Some("/usr/bin:/bin"));
    }
}
