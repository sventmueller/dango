//! The register of running processes: who started them, how to end them, and who wants to hear when that changes.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// What a process is there for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A shell in a terminal tab.
    Terminal,
    /// A language server.
    LanguageServer,
    /// A coding agent or its adapter.
    Agent,
    /// A web server a project runs while it is developed.
    DevServer,
    /// git.
    Git,
    /// Anything else, such as an installer.
    Tool,
}

/// How the supervisor knows a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Started by the supervisor, which waits for its end.
    Started,
    /// Started elsewhere, such as a terminal's shell, and reported by its owner.
    Registered,
}

/// The number the supervisor gives a process; never reused while the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(u64);

/// A running process as the activity view shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    /// The supervisor's number.
    pub id: ProcessId,
    /// The system's process number, which is also the number of its process group.
    pub pid: u32,
    /// What it is there for.
    pub kind: Kind,
    /// What the user reads, such as `gopls` or `claude`.
    pub label: String,
    /// When it started.
    pub started: SystemTime,
    /// How the supervisor knows it.
    pub origin: Origin,
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// It exited with this code.
    Code(i32),
    /// A signal ended it.
    Signal(i32),
    /// Its owner reported it gone, or waiting for it failed.
    Unknown,
}

/// A change to the register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A process started or was registered.
    Started(ProcessInfo),
    /// A process ended or was forgotten.
    Ended {
        /// The process.
        id: ProcessId,
        /// How it ended.
        ending: Ending,
    },
}

/// The pipes of a started process; what the command did not ask for is `None`.
pub struct Spawned {
    /// The supervisor's number for it.
    pub id: ProcessId,
    /// Its standard input.
    pub stdin: Option<ChildStdin>,
    /// Its standard output.
    pub stdout: Option<ChildStdout>,
    /// Its standard error.
    pub stderr: Option<ChildStderr>,
}

type Listener = Arc<dyn Fn(&Event) + Send + Sync>;
type GracefulStop = Box<dyn FnOnce() + Send>;

struct Entry {
    info: ProcessInfo,
    graceful: Option<GracefulStop>,
}

/// How many endings `wait` can still report after the process is gone.
const ENDINGS_KEPT: usize = 256;

#[derive(Default)]
struct Register {
    next: u64,
    entries: BTreeMap<ProcessId, Entry>,
    listeners: Vec<Listener>,
    ended: VecDeque<(ProcessId, Ending)>,
}

/// Keeps every process the app starts, each in a process group of its own so that ending it also ends what it started.
///
/// ```no_run
/// use std::process::{Command, Stdio};
/// use std::time::Duration;
///
/// use ide_process::{Ending, Kind, Supervisor};
///
/// let supervisor = Supervisor::new();
/// let mut command = Command::new("/bin/sleep");
/// command.arg("60").stdin(Stdio::null());
/// let spawned = supervisor.spawn(command, Kind::Tool, "sleep")?;
/// assert_eq!(supervisor.running()[0].label, "sleep");
/// supervisor.stop(spawned.id, Duration::from_millis(200));
/// assert_eq!(supervisor.wait(spawned.id, Duration::from_secs(5)), Some(Ending::Signal(libc::SIGTERM)));
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Clone, Default)]
pub struct Supervisor {
    register: Arc<(Mutex<Register>, Condvar)>,
}

impl Supervisor {
    /// A supervisor of its own, for tests; the app uses `global`.
    pub fn new() -> Supervisor {
        Supervisor::default()
    }

    /// The app's supervisor.
    pub fn global() -> &'static Supervisor {
        static GLOBAL: OnceLock<Supervisor> = OnceLock::new();
        GLOBAL.get_or_init(Supervisor::new)
    }

    /// Starts `command` in a new process group and keeps it until it ends.
    pub fn spawn(&self, mut command: Command, kind: Kind, label: impl Into<String>) -> io::Result<Spawned> {
        command.process_group(0);
        let mut child = command.spawn()?;
        let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let info = self.add(child.id(), kind, label.into(), Origin::Started);
        let id = info.id;
        let supervisor = self.clone();
        std::thread::Builder::new()
            .name(format!("wait-{}", child.id()))
            .spawn(move || supervisor.wait_for(id, child))?;
        self.announce(&Event::Started(info));
        Ok(Spawned {
            id,
            stdin,
            stdout,
            stderr,
        })
    }

    /// Adds a process started elsewhere, which leads its own process group; its owner calls `forget` when it ends.
    pub fn register(&self, pid: u32, kind: Kind, label: impl Into<String>) -> ProcessId {
        let info = self.add(pid, kind, label.into(), Origin::Registered);
        let id = info.id;
        self.announce(&Event::Started(info));
        id
    }

    /// Removes a registered process that has ended.
    pub fn forget(&self, id: ProcessId) {
        self.remove(id, Ending::Unknown);
    }

    /// What `stop` does first for `id`, such as closing its input or asking a language server to shut down.
    pub fn on_stop(&self, id: ProcessId, graceful: impl FnOnce() + Send + 'static) {
        let (lock, _) = &*self.register;
        if let Some(entry) = lock.lock().unwrap_or_else(|e| e.into_inner()).entries.get_mut(&id) {
            entry.graceful = Some(Box::new(graceful));
        }
    }

    /// Calls `listener` with every change from now on, on whichever thread makes it.
    pub fn subscribe(&self, listener: impl Fn(&Event) + Send + Sync + 'static) {
        let (lock, _) = &*self.register;
        lock.lock()
            .unwrap_or_else(|e| e.into_inner())
            .listeners
            .push(Arc::new(listener));
    }

    /// How `id` ended, waiting up to `timeout` for it; `None` while it still runs.
    pub fn wait(&self, id: ProcessId, timeout: Duration) -> Option<Ending> {
        let deadline = Instant::now() + timeout;
        let (lock, changed) = &*self.register;
        let mut register = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some((_, ending)) = register.ended.iter().find(|(ended, _)| *ended == id) {
                return Some(*ending);
            }
            if !register.entries.contains_key(&id) {
                return Some(Ending::Unknown);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            register = changed
                .wait_timeout(register, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// The processes that run now, oldest first.
    pub fn running(&self) -> Vec<ProcessInfo> {
        let (lock, _) = &*self.register;
        lock.lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .values()
            .map(|e| e.info.clone())
            .collect()
    }

    /// Ends `id` in the background: its graceful step, then after `grace` SIGTERM to its group, then after another
    /// `grace` SIGKILL.
    pub fn stop(&self, id: ProcessId, grace: Duration) {
        let supervisor = self.clone();
        let _ = std::thread::Builder::new()
            .name("stop".into())
            .spawn(move || supervisor.end(&[id], grace));
    }

    /// Ends every process as `stop` does and returns once all are gone or `grace` has passed three times.
    pub fn shutdown(&self, grace: Duration) {
        let ids: Vec<ProcessId> = self.running().iter().map(|info| info.id).collect();
        self.end(&ids, grace);
    }

    fn end(&self, ids: &[ProcessId], grace: Duration) {
        let mut graceful = vec![];
        {
            let (lock, _) = &*self.register;
            let mut register = lock.lock().unwrap_or_else(|e| e.into_inner());
            for id in ids {
                if let Some(step) = register.entries.get_mut(id).and_then(|e| e.graceful.take()) {
                    graceful.push(step);
                }
            }
        }
        let had_graceful = !graceful.is_empty();
        for step in graceful {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(step));
        }
        if had_graceful && self.wait_until_gone(ids, grace) {
            return;
        }
        self.signal(ids, libc::SIGTERM);
        if self.wait_until_gone(ids, grace) {
            return;
        }
        self.signal(ids, libc::SIGKILL);
        if !self.wait_until_gone(ids, grace) {
            // A registered process whose owner never reports its end would otherwise stay listed for good.
            for id in ids {
                if self.origin(*id) == Some(Origin::Registered) {
                    self.forget(*id);
                }
            }
        }
    }

    fn origin(&self, id: ProcessId) -> Option<Origin> {
        let (lock, _) = &*self.register;
        lock.lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .get(&id)
            .map(|e| e.info.origin)
    }

    fn signal(&self, ids: &[ProcessId], signal: i32) {
        let (lock, _) = &*self.register;
        let register = lock.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            let Some(entry) = register.entries.get(id) else {
                continue;
            };
            let Ok(group) = libc::pid_t::try_from(entry.info.pid) else {
                continue;
            };
            // SAFETY: kill only sends a signal; the group is the one the process leads, still listed and so not yet
            // reaped, which keeps its number from being reused.
            unsafe { libc::kill(-group, signal) };
        }
    }

    /// Whether all of `ids` ended within `timeout`.
    fn wait_until_gone(&self, ids: &[ProcessId], timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let (lock, changed) = &*self.register;
        loop {
            self.prune_registered();
            let register = lock.lock().unwrap_or_else(|e| e.into_inner());
            if ids.iter().all(|id| !register.entries.contains_key(id)) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            // Registered processes end without a notice to the supervisor, so it looks again now and then.
            let wait = (deadline - now).min(Duration::from_millis(50));
            drop(changed.wait_timeout(register, wait));
        }
    }

    /// Drops registered processes that no longer exist.
    fn prune_registered(&self) {
        let (lock, changed) = &*self.register;
        let gone: Vec<ProcessId> = {
            let mut register = lock.lock().unwrap_or_else(|e| e.into_inner());
            let gone: Vec<ProcessId> = register
                .entries
                .values()
                .filter(|e| e.info.origin == Origin::Registered && !exists(e.info.pid))
                .map(|e| e.info.id)
                .collect();
            for id in &gone {
                register.entries.remove(id);
            }
            gone
        };
        if !gone.is_empty() {
            changed.notify_all();
        }
        for id in gone {
            self.announce(&Event::Ended {
                id,
                ending: Ending::Unknown,
            });
        }
    }

    fn add(&self, pid: u32, kind: Kind, label: String, origin: Origin) -> ProcessInfo {
        let (lock, _) = &*self.register;
        let mut register = lock.lock().unwrap_or_else(|e| e.into_inner());
        register.next += 1;
        let info = ProcessInfo {
            id: ProcessId(register.next),
            pid,
            kind,
            label,
            started: SystemTime::now(),
            origin,
        };
        register.entries.insert(
            info.id,
            Entry {
                info: info.clone(),
                graceful: None,
            },
        );
        info
    }

    fn wait_for(&self, id: ProcessId, mut child: Child) {
        let ending = match child.wait() {
            Ok(status) => match (status.code(), status.signal()) {
                (Some(code), _) => Ending::Code(code),
                (None, Some(signal)) => Ending::Signal(signal),
                (None, None) => Ending::Unknown,
            },
            Err(_) => Ending::Unknown,
        };
        self.remove(id, ending);
    }

    fn remove(&self, id: ProcessId, ending: Ending) {
        let (lock, changed) = &*self.register;
        let removed = {
            let mut register = lock.lock().unwrap_or_else(|e| e.into_inner());
            let removed = register.entries.remove(&id).is_some();
            if removed {
                if register.ended.len() == ENDINGS_KEPT {
                    register.ended.pop_front();
                }
                register.ended.push_back((id, ending));
            }
            removed
        };
        changed.notify_all();
        if removed {
            self.announce(&Event::Ended { id, ending });
        }
    }

    fn announce(&self, event: &Event) {
        let (lock, _) = &*self.register;
        let listeners = lock.lock().unwrap_or_else(|e| e.into_inner()).listeners.clone();
        for listener in listeners {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(event)));
        }
    }
}

/// Whether a process numbered `pid` exists.
fn exists(pid: u32) -> bool {
    // SAFETY: Signal 0 only asks whether the process exists; nothing is delivered.
    libc::pid_t::try_from(pid).is_ok_and(|pid| unsafe { libc::kill(pid, 0) } == 0)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::sync::mpsc::channel;

    use super::*;

    const GRACE: Duration = Duration::from_secs(2);

    fn sleeper(seconds: u32) -> Command {
        let mut command = Command::new("/bin/sleep");
        command.arg(seconds.to_string());
        command
    }

    #[test]
    fn a_process_is_listed_until_it_exits_and_its_code_is_reported() {
        let supervisor = Supervisor::new();
        let (tx, rx) = channel();
        supervisor.subscribe(move |event| tx.send(event.clone()).unwrap());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 3"]);
        let spawned = supervisor.spawn(command, Kind::Tool, "sh").unwrap();
        let started = rx.recv_timeout(GRACE).unwrap();
        assert!(matches!(started, Event::Started(ref info) if info.id == spawned.id && info.label == "sh"));
        let ended = rx.recv_timeout(GRACE).unwrap();
        assert_eq!(
            ended,
            Event::Ended {
                id: spawned.id,
                ending: Ending::Code(3)
            }
        );
        assert!(supervisor.running().is_empty());
    }

    #[test]
    fn waiting_reports_how_a_process_ended_also_after_it_is_gone() {
        let supervisor = Supervisor::new();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 0.2; exit 4"]);
        let spawned = supervisor.spawn(command, Kind::Tool, "sh").unwrap();
        assert_eq!(supervisor.wait(spawned.id, Duration::from_millis(10)), None);
        assert_eq!(supervisor.wait(spawned.id, GRACE), Some(Ending::Code(4)));
        assert_eq!(supervisor.wait(spawned.id, Duration::ZERO), Some(Ending::Code(4)));
    }

    #[test]
    fn stopping_ends_the_whole_group_including_what_the_process_started() {
        let supervisor = Supervisor::new();
        let mut command = Command::new("/bin/sh");
        // The grandchild prints its number, then outlives the shell unless its group is ended.
        command.args(["-c", "sleep 30 & echo $!; wait"]).stdout(Stdio::piped());
        let spawned = supervisor.spawn(command, Kind::Agent, "parent").unwrap();
        let mut line = String::new();
        BufReader::new(spawned.stdout.unwrap()).read_line(&mut line).unwrap();
        let grandchild: u32 = line.trim().parse().unwrap();
        assert!(exists(grandchild));
        supervisor.stop(spawned.id, GRACE);
        let deadline = Instant::now() + GRACE * 3;
        while exists(grandchild) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!exists(grandchild), "the grandchild survived");
    }

    #[test]
    fn the_graceful_step_comes_first_and_can_be_enough() {
        let supervisor = Supervisor::new();
        let mut command = Command::new("/bin/cat");
        command.stdin(Stdio::piped());
        let spawned = supervisor.spawn(command, Kind::LanguageServer, "cat").unwrap();
        let stdin = Mutex::new(spawned.stdin);
        let (tx, rx) = channel();
        supervisor.subscribe(move |event| {
            if let Event::Ended { ending, .. } = event {
                tx.send(*ending).unwrap();
            }
        });
        supervisor.on_stop(spawned.id, move || drop(stdin.lock().unwrap().take()));
        supervisor.shutdown(GRACE);
        assert_eq!(
            rx.recv_timeout(GRACE).unwrap(),
            Ending::Code(0),
            "cat ended by itself at the end of its input"
        );
    }

    #[test]
    fn shutdown_ends_everything_even_what_ignores_sigterm() {
        let supervisor = Supervisor::new();
        let mut stubborn = Command::new("/bin/sh");
        stubborn.args(["-c", "trap '' TERM; while true; do sleep 1; done"]);
        supervisor.spawn(stubborn, Kind::DevServer, "stubborn").unwrap();
        supervisor.spawn(sleeper(30), Kind::Tool, "sleep").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let started = Instant::now();
        supervisor.shutdown(Duration::from_millis(300));
        assert!(supervisor.running().is_empty());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_registered_process_leaves_the_list_when_it_is_gone() {
        let supervisor = Supervisor::new();
        let mut child = sleeper(30).process_group(0).spawn().unwrap();
        let id = supervisor.register(child.id(), Kind::Terminal, "zsh");
        assert_eq!(supervisor.running().len(), 1);
        assert_eq!(supervisor.running()[0].origin, Origin::Registered);
        let waiter = std::thread::spawn(move || child.wait());
        supervisor.stop(id, Duration::from_millis(200));
        waiter.join().unwrap().unwrap();
        let deadline = Instant::now() + GRACE;
        while !supervisor.running().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(supervisor.running().is_empty());
    }
}
