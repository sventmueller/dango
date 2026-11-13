//! One git process per call, through the supervisor, with the user's environment, limits on what it may print, and an
//! end that can be waited for, cancelled or timed out.

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ide_process::tail::Tail;
use ide_process::{Ending, Event, Kind, ProcessId, Spawned, Supervisor};

use crate::error::{Error, Result};

/// How much of standard error is kept.
const STDERR_LIMIT: usize = 64 * 1024;

/// How much of standard output is read before the call fails; diffs and patches are useless when cut.
const STDOUT_LIMIT: usize = 256 * 1024 * 1024;

/// The limit for commands that only read or write the local repository.
pub(crate) const LOCAL_TIMEOUT: Duration = Duration::from_secs(300);

/// How long a stopped git gets for each step from SIGTERM to SIGKILL.
const STOP_GRACE: Duration = Duration::from_millis(500);

/// SIGHUP, SIGINT, SIGKILL and SIGTERM: someone stopped git, it did not crash.
const STOP_SIGNALS: [i32; 4] = [1, 2, 9, 15];

/// How long the end of a process nobody waits for is kept.
const KEEP_ENDINGS: Duration = Duration::from_secs(60);

/// Variables that tie git to one repository; `git rev-parse --local-env-vars` lists them.
const REPOSITORY_VARIABLES: [&str; 15] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// Variables this crate sets itself.
const OWN_VARIABLES: [&str; 10] = [
    "GIT_TERMINAL_PROMPT",
    "GIT_TRACE2_PARENT_SID",
    "GIT_TRACE2_PARENT_NAME",
    "GIT_EDITOR",
    "GIT_MERGE_AUTOEDIT",
    "GIT_TRACE2_EVENT",
    "LC_ALL",
    "LANG",
    "LANGUAGE",
    "GIT_PAGER",
];

pub use ide_core::Cancel;

/// The app's way of running git: the program from the login shell's PATH, that shell's environment, and the
/// supervisor every process goes through. Clones share one listener on the supervisor, so make one and clone it.
#[derive(Clone)]
pub struct Git {
    inner: Arc<Inner>,
}

struct Inner {
    program: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
    supervisor: Supervisor,
    endings: Arc<Endings>,
}

impl std::fmt::Debug for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Git")
            .field("program", &self.inner.program)
            .finish_non_exhaustive()
    }
}

impl Git {
    /// git with the login shell's `environment`, which carries PATH, `SSH_AUTH_SOCK` and what credential helpers need.
    pub fn new(supervisor: &Supervisor, environment: Vec<(String, String)>) -> Git {
        let environment = prepared(environment);
        let program = find_program(&environment, "git").or_else(|| {
            let fallback = PathBuf::from("/usr/bin/git");
            fallback.exists().then_some(fallback)
        });
        let endings = Arc::new(Endings::default());
        let weak: Weak<Endings> = Arc::downgrade(&endings);
        supervisor.subscribe(move |event| {
            if let (Event::Ended { id, ending }, Some(endings)) = (event, weak.upgrade()) {
                endings.record(*id, *ending);
            }
        });
        Git {
            inner: Arc::new(Inner {
                program,
                environment,
                supervisor: supervisor.clone(),
                endings,
            }),
        }
    }

    /// The git program in use, if one was found.
    pub fn program(&self) -> Option<&Path> {
        self.inner.program.as_deref()
    }

    /// git's version, such as `git version 2.50.1`; `GitMissing` when git cannot run.
    pub fn version(&self) -> Result<String> {
        let dir = std::env::temp_dir();
        let output = self.ok(Call::new(&dir, ["--version"]))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// The program called `name` on the login shell's PATH.
    pub(crate) fn find(&self, name: &str) -> Option<PathBuf> {
        find_program(&self.inner.environment, name)
    }

    /// Runs `call` and fails unless git exits with 0.
    pub(crate) fn ok(&self, call: Call<'_>) -> Result<Output> {
        let output = self.run(call)?;
        if output.code == 0 {
            Ok(output)
        } else {
            Err(failure(&output))
        }
    }

    /// Runs `call` to its end; the exit code is the caller's to judge.
    pub(crate) fn run(&self, call: Call<'_>) -> Result<Output> {
        let program = self.inner.program.as_ref().ok_or(Error::GitMissing)?;
        self.run_program(program, call)
    }

    /// Runs another program the same way, such as gitleaks.
    pub(crate) fn run_program(&self, program: &Path, call: Call<'_>) -> Result<Output> {
        let mut command = Command::new(program);
        command
            .args(&call.args)
            .current_dir(call.dir)
            .env_clear()
            .envs(self.inner.environment.iter().map(|(k, v)| (k, v)))
            .envs(call.env.iter().map(|(k, v)| (k, v)))
            .stdin(if call.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let kind = if program.file_name() == Some(OsStr::new("git")) {
            Kind::Git
        } else {
            Kind::Tool
        };
        let label = label(program, &call.args);
        let spawned = match self.inner.supervisor.spawn(command, kind, label.clone()) {
            Ok(spawned) => spawned,
            Err(error) if error.kind() == io::ErrorKind::NotFound && !program.exists() => {
                return Err(Error::GitMissing);
            }
            Err(error) => return Err(Error::Io(error)),
        };
        self.inner.endings.expect(spawned.id);
        self.collect(spawned, call, label)
    }

    fn collect(&self, spawned: Spawned, mut call: Call<'_>, command: String) -> Result<Output> {
        let id = spawned.id;
        let (rx, mut streams, stdout, tail) = match readers(spawned, &mut call) {
            Ok(readers) => readers,
            Err(error) => {
                // The process already runs; without its readers nobody would end it or collect its end.
                self.inner.supervisor.stop(id, Duration::from_millis(200));
                self.inner.endings.forget(id);
                return Err(Error::Io(error));
            }
        };
        let deadline = call.timeout.map(|t| Instant::now() + t);
        let mut stopped = None;
        while streams > 0 {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Message::Segment(text)) => {
                    if let Some(progress) = call.progress.as_mut() {
                        progress(&text);
                    }
                }
                Ok(Message::Done) => streams -= 1,
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.check_stop(id, &call, deadline, &mut stopped);
        }
        let ending = loop {
            if let Some(ending) = self.inner.endings.wait(id, Duration::from_millis(50)) {
                break ending;
            }
            self.check_stop(id, &call, deadline, &mut stopped);
        };
        let (stdout, overflow) = stdout.map(|h| h.join().unwrap_or_default()).unwrap_or_default();
        if let Some(error) = stopped {
            return Err(error);
        }
        let stderr = tail.lock_ignoring_poison().text();
        let code = match ending {
            Ending::Code(code) => code,
            Ending::Signal(signal) if STOP_SIGNALS.contains(&signal) => {
                return Err(Error::Cancelled);
            }
            Ending::Signal(_) | Ending::Unknown => {
                return Err(Error::Failed {
                    command,
                    code: None,
                    output: stderr,
                });
            }
        };
        if overflow {
            return Err(Error::OutputTooLarge);
        }
        Ok(Output {
            code,
            stdout,
            stderr,
            command,
            dir: call.dir.to_path_buf(),
        })
    }

    fn check_stop(&self, id: ProcessId, call: &Call<'_>, deadline: Option<Instant>, stopped: &mut Option<Error>) {
        if stopped.is_some() {
            return;
        }
        if call.cancel.is_some_and(Cancel::is_cancelled) {
            *stopped = Some(Error::Cancelled);
        } else if deadline.is_some_and(|d| Instant::now() >= d) {
            *stopped = Some(Error::TimedOut);
        } else {
            return;
        }
        self.inner.supervisor.stop(id, STOP_GRACE);
    }
}

/// One git command: its arguments, folder, extra environment, input, limits and listeners.
pub(crate) struct Call<'a> {
    args: Vec<OsString>,
    dir: &'a Path,
    env: Vec<(OsString, OsString)>,
    input: Option<Vec<u8>>,
    timeout: Option<Duration>,
    cancel: Option<&'a Cancel>,
    progress: Option<&'a mut dyn FnMut(&str)>,
}

impl<'a> Call<'a> {
    /// `git <args>` in `dir`, with the local time limit.
    pub(crate) fn new<I, S>(dir: &'a Path, args: I) -> Call<'a>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Call {
            args: args.into_iter().map(|a| a.as_ref().to_os_string()).collect(),
            dir,
            env: Vec::new(),
            input: None,
            timeout: Some(LOCAL_TIMEOUT),
            cancel: None,
            progress: None,
        }
    }

    /// Adds arguments.
    pub(crate) fn args<I, S>(mut self, args: I) -> Call<'a>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args.extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    /// Sets an environment variable for this call.
    pub(crate) fn env(mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Call<'a> {
        self.env
            .push((name.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    /// Sets the variables of `env` for this call.
    pub(crate) fn envs(mut self, env: &[(OsString, OsString)]) -> Call<'a> {
        self.env.extend_from_slice(env);
        self
    }

    /// Writes `input` to git's standard input.
    pub(crate) fn input(mut self, input: Vec<u8>) -> Call<'a> {
        self.input = Some(input);
        self
    }

    /// Lets the call run as long as it takes; for network operations and hooks, which can be cancelled instead.
    pub(crate) fn unlimited(mut self) -> Call<'a> {
        self.timeout = None;
        self
    }

    /// Stops the call when `cancel` is requested.
    pub(crate) fn cancel(mut self, cancel: &'a Cancel) -> Call<'a> {
        self.cancel = Some(cancel);
        self
    }

    /// Hands each line or carriage-return-separated piece of standard error to `progress` as it arrives.
    pub(crate) fn progress(mut self, progress: &'a mut dyn FnMut(&str)) -> Call<'a> {
        self.progress = Some(progress);
        self
    }
}

/// What a finished git printed.
#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) code: i32,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
    pub(crate) command: String,
    pub(crate) dir: PathBuf,
}

impl Output {
    /// Standard output as text, trimmed.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }

    /// Standard error, then standard output, for errors that show what git said.
    pub(crate) fn said(&self) -> String {
        let stdout = String::from_utf8_lossy(&self.stdout);
        match (self.stderr.trim().is_empty(), stdout.trim().is_empty()) {
            (_, true) => self.stderr.clone(),
            (true, false) => stdout.into_owned(),
            (false, false) => format!("{}\n{}", self.stderr.trim_end(), stdout.trim_end()),
        }
    }
}

/// The typed error for a git that failed; specific commands look for their own cases first.
pub(crate) fn failure(output: &Output) -> Error {
    let said = output.said();
    let lower = said.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(&[
        "xcrun: error",
        "invalid active developer path",
        "no developer tools were found",
    ]) {
        Error::GitMissing
    } else if has(&["not a git repository"]) {
        Error::NotARepository {
            path: output.dir.clone(),
        }
    } else if has(&["detected dubious ownership"]) {
        Error::UnsafeRepository {
            path: output.dir.clone(),
            output: said,
        }
    } else if has(&[
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "permission denied (publickey",
        "invalid username or password",
        "http basic: access denied",
        "returned error: 401",
        "returned error: 403",
        "host key verification failed",
        "repository not found",
    ]) {
        Error::AuthFailed { output: said }
    } else if has(&[
        "could not resolve host",
        "could not resolve hostname",
        "unable to access",
        "connection refused",
        "connection timed out",
        "operation timed out",
        "network is unreachable",
        "no route to host",
        "the remote end hung up unexpectedly",
        "could not read from remote repository",
        "failed to connect",
        "early eof",
    ]) {
        Error::Network { output: said }
    } else {
        Error::Failed {
            command: output.command.clone(),
            code: Some(output.code),
            output: said,
        }
    }
}

enum Message {
    Segment(String),
    Done,
}

/// What the process's streams arrive on, how many there are, the stdout reader and the stderr tail.
type Readers = (
    Receiver<Message>,
    usize,
    Option<JoinHandle<(Vec<u8>, bool)>>,
    Arc<Mutex<Tail>>,
);

/// Starts a thread for each of the process's streams.
fn readers(spawned: Spawned, call: &mut Call<'_>) -> io::Result<Readers> {
    let (tx, rx) = mpsc::channel::<Message>();
    let mut streams = 0;
    if let (Some(mut stdin), Some(input)) = (spawned.stdin, call.input.take()) {
        std::thread::Builder::new()
            .name("git-stdin".into())
            .spawn(move || drop(stdin.write_all(&input)))?;
    }
    let stdout = match spawned.stdout {
        Some(out) => {
            streams += 1;
            let tx = tx.clone();
            Some(std::thread::Builder::new().name("git-stdout".into()).spawn(move || {
                let read = read_limited(out, STDOUT_LIMIT);
                let _ = tx.send(Message::Done);
                read
            })?)
        }
        None => None,
    };
    let tail = Arc::new(Mutex::new(Tail::new(STDERR_LIMIT)));
    if let Some(err) = spawned.stderr {
        streams += 1;
        let (tail, tx, segments) = (tail.clone(), tx.clone(), call.progress.is_some());
        std::thread::Builder::new()
            .name("git-stderr".into())
            .spawn(move || read_stderr(err, &tail, &tx, segments))?;
    }
    Ok((rx, streams, stdout, tail))
}

/// The ends of processes as the supervisor reports them, kept until their caller takes them.
#[derive(Default)]
struct Endings {
    state: Mutex<EndingState>,
    changed: Condvar,
}

#[derive(Default)]
struct EndingState {
    ended: HashMap<ProcessId, (Ending, Instant)>,
    expected: HashSet<ProcessId>,
}

impl Endings {
    /// Notes that a caller will wait for `id`, so its end is kept however long it takes.
    fn expect(&self, id: ProcessId) {
        self.state.lock_ignoring_poison().expected.insert(id);
    }

    fn record(&self, id: ProcessId, ending: Ending) {
        let now = Instant::now();
        let mut state = self.state.lock_ignoring_poison();
        let EndingState { ended, expected } = &mut *state;
        ended.retain(|id, (_, at)| expected.contains(id) || now.duration_since(*at) < KEEP_ENDINGS);
        ended.insert(id, (ending, now));
        drop(state);
        self.changed.notify_all();
    }

    /// Stops keeping the end of `id`, which nobody waits for any more.
    fn forget(&self, id: ProcessId) {
        let mut state = self.state.lock_ignoring_poison();
        state.expected.remove(&id);
        state.ended.remove(&id);
    }

    /// The end of `id`, waiting up to `limit` for it.
    fn wait(&self, id: ProcessId, limit: Duration) -> Option<Ending> {
        let deadline = Instant::now() + limit;
        let mut state = self.state.lock_ignoring_poison();
        loop {
            if let Some((ending, _)) = state.ended.remove(&id) {
                state.expected.remove(&id);
                return Some(ending);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            state = self.changed.wait_timeout(state, deadline - now).ignoring_poison().0;
        }
    }
}

/// The login shell's environment without what would point git elsewhere, plus what keeps git from waiting for a
/// person who is not there.
fn prepared(environment: Vec<(String, String)>) -> Vec<(OsString, OsString)> {
    let mut prepared: Vec<(OsString, OsString)> = environment
        .into_iter()
        .filter(|(name, _)| !REPOSITORY_VARIABLES.contains(&name.as_str()) && !OWN_VARIABLES.contains(&name.as_str()))
        .map(|(name, value)| (name.into(), value.into()))
        .collect();
    for (name, value) in [
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_EDITOR", "true"),
        ("GIT_MERGE_AUTOEDIT", "no"),
        ("GIT_PAGER", "cat"),
        // Errors are told apart by git's English messages; a UTF-8 locale keeps hooks reading files as before.
        ("LC_ALL", "en_US.UTF-8"),
        ("LANG", "en_US.UTF-8"),
    ] {
        prepared.push((name.into(), value.into()));
    }
    prepared
}

/// The executable `name` in the PATH of `environment`.
fn find_program(environment: &[(OsString, OsString)], name: &str) -> Option<PathBuf> {
    let path = environment.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.clone())?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// What the activity view shows, such as `git status`.
fn label(program: &Path, args: &[OsString]) -> String {
    let name = program
        .file_name()
        .map_or_else(|| "git".into(), |n| n.to_string_lossy().into_owned());
    let mut args = args.iter().map(|a| a.to_string_lossy());
    let mut words = vec![name];
    while let Some(arg) = args.next() {
        if arg == "-c" || arg == "-C" {
            args.next();
        } else if !arg.starts_with('-') {
            words.push(arg.into_owned());
            break;
        }
    }
    words.join(" ")
}

/// Reads `reader` to its end, keeping up to `limit` bytes; the flag tells whether more came.
fn read_limited(mut reader: impl Read, limit: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut overflow = false;
    let mut buffer = [0u8; 64 * 1024];
    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }
        if kept.len() + n > limit {
            overflow = true;
        } else {
            kept.extend_from_slice(&buffer[..n]);
        }
    }
    (kept, overflow)
}

/// Reads standard error into `tail`, and sends each piece between line ends or carriage returns when asked to.
fn read_stderr(mut reader: impl Read, tail: &Mutex<Tail>, tx: &Sender<Message>, segments: bool) {
    let mut buffer = [0u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }
        tail.lock_ignoring_poison().push(&buffer[..n]);
        if !segments {
            continue;
        }
        for &byte in &buffer[..n] {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    let _ = tx.send(Message::Segment(String::from_utf8_lossy(&pending).into_owned()));
                    pending.clear();
                }
            } else if pending.len() < 4096 {
                pending.push(byte);
            }
        }
    }
    if segments && !pending.is_empty() {
        let _ = tx.send(Message::Segment(String::from_utf8_lossy(&pending).into_owned()));
    }
    let _ = tx.send(Message::Done);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_loses_repository_pointers_and_gains_no_prompts() {
        let prepared = prepared(vec![
            ("PATH".into(), "/usr/bin".into()),
            ("GIT_DIR".into(), "/elsewhere".into()),
            ("LANGUAGE".into(), "de".into()),
            ("GIT_TERMINAL_PROMPT".into(), "1".into()),
            ("SSH_AUTH_SOCK".into(), "/tmp/agent".into()),
        ]);
        let get = |name: &str| {
            prepared
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.to_string_lossy().into_owned())
        };
        assert_eq!(get("GIT_DIR"), None);
        assert_eq!(get("LANGUAGE"), None);
        assert_eq!(get("GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
        assert_eq!(get("SSH_AUTH_SOCK").as_deref(), Some("/tmp/agent"));
        assert_eq!(prepared.iter().filter(|(k, _)| k == "GIT_TERMINAL_PROMPT").count(), 1);
    }

    #[test]
    fn the_label_names_the_subcommand() {
        let args: Vec<OsString> = ["-c", "core.quotepath=false", "--no-optional-locks", "status", "-z"]
            .iter()
            .map(OsString::from)
            .collect();
        assert_eq!(label(Path::new("/usr/bin/git"), &args), "git status");
    }

    #[test]
    fn output_beyond_the_limit_is_flagged() {
        let (kept, overflow) = read_limited(&b"0123456789"[..], 4);
        assert!(overflow);
        assert!(kept.len() <= 4);
        let (kept, overflow) = read_limited(&b"0123"[..], 4);
        assert_eq!((kept.as_slice(), overflow), (&b"0123"[..], false));
    }

    #[test]
    fn standard_error_is_split_at_carriage_returns_and_line_ends() {
        let tail = Mutex::new(Tail::new(1024));
        let (tx, rx) = mpsc::channel();
        read_stderr(
            &b"Counting:  50% (1/2)\rCounting: 100% (2/2), done.\nlast"[..],
            &tail,
            &tx,
            true,
        );
        let segments: Vec<String> = rx
            .try_iter()
            .filter_map(|m| match m {
                Message::Segment(s) => Some(s),
                Message::Done => None,
            })
            .collect();
        assert_eq!(
            segments,
            ["Counting:  50% (1/2)", "Counting: 100% (2/2), done.", "last"]
        );
    }

    #[test]
    fn failures_are_sorted_by_what_git_said() {
        let output = |stderr: &str| Output {
            code: 128,
            stdout: Vec::new(),
            stderr: stderr.into(),
            command: "git fetch".into(),
            dir: PathBuf::from("/r"),
        };
        assert!(matches!(
            failure(&output(
                "fatal: could not read Username for 'https://x': terminal prompts disabled"
            )),
            Error::AuthFailed { .. }
        ));
        assert!(matches!(
            failure(&output("ssh: Could not resolve hostname x")),
            Error::Network { .. }
        ));
        assert!(matches!(
            failure(&output(
                "fatal: not a git repository (or any of the parent directories): .git"
            )),
            Error::NotARepository { .. }
        ));
        assert!(matches!(
            failure(&output("fatal: something else")),
            Error::Failed { code: Some(128), .. }
        ));
    }
}
