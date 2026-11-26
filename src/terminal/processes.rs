//! The terminal crates' child processes in the app's supervisor (E10, E23): the shells as `Kind::Terminal`, which
//! their session waits for itself, and the programs the suggestions ask, such as `docker ps`, from the login shell's
//! `PATH` with its environment and within their deadline.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ide_core::LockIgnoringPoison;
use ide_process::tool::Helper;
use ide_process::{Kind, ProcessId, Supervisor};

/// The supervisor as the spawner of a session; its own numbers stand for the supervisor's ids, which no other crate can
/// make.
pub(crate) struct Spawner {
    supervisor: Supervisor,
    next: AtomicU64,
    supervised: Mutex<HashMap<u64, ProcessId>>,
}

impl Default for Spawner {
    fn default() -> Spawner {
        Spawner::new(Supervisor::global().clone())
    }
}

impl Spawner {
    fn new(supervisor: Supervisor) -> Spawner {
        Spawner {
            supervisor,
            next: AtomicU64::new(0),
            supervised: Mutex::new(HashMap::new()),
        }
    }

    fn supervised(&self, id: term_base::ProcessId) -> Option<ProcessId> {
        self.supervised.lock_ignoring_poison().get(&id.0).copied()
    }
}

impl term_base::Spawner for Spawner {
    fn register(&self, pid: u32, label: &str) -> term_base::ProcessId {
        let supervised = self.supervisor.register(pid, Kind::Terminal, label);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.supervised.lock_ignoring_poison().insert(id, supervised);
        term_base::ProcessId(id)
    }

    fn on_stop(&self, id: term_base::ProcessId, stop: term_base::OnStop) {
        if let Some(supervised) = self.supervised(id) {
            self.supervisor.on_stop(supervised, stop);
        }
    }

    fn stop(&self, id: term_base::ProcessId, grace: Duration) {
        if let Some(supervised) = self.supervised(id) {
            self.supervisor.stop(supervised, grace);
        }
    }

    fn forget(&self, id: term_base::ProcessId) {
        if let Some(supervised) = self.supervised.lock_ignoring_poison().remove(&id.0) {
            self.supervisor.forget(supervised);
        }
    }
}

/// The programs of the suggestions, run as the app runs its helpers.
pub(crate) struct Programs;

impl term_suggest::Programs for Programs {
    fn path(&self) -> Option<OsString> {
        ide_process::tool::var(crate::environment::get()?, "PATH").map(OsString::from)
    }

    fn output(&self, program: &Path, args: &[&str], folder: &Path, deadline: Instant) -> Option<String> {
        let helper = Helper {
            supervisor: Supervisor::global(),
            env: crate::environment::get()?,
            timeout: deadline.saturating_duration_since(Instant::now()),
            cancel: None,
        };
        let output = helper.run(program, args, folder, &mut |_| {}).ok()?;
        output.succeeded().then_some(output.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_base::Spawner as _;

    #[test]
    fn a_shell_is_listed_as_a_terminal_until_its_session_forgets_it() {
        let supervisor = Supervisor::new();
        let spawner = Spawner::new(supervisor.clone());
        let pid = std::process::id();
        let id = spawner.register(pid, "zsh");
        let listed = supervisor.running();
        assert!(
            listed
                .iter()
                .any(|process| process.pid == pid && process.kind == Kind::Terminal && process.label == "zsh"),
            "{listed:?}"
        );
        spawner.forget(id);
        assert!(supervisor.running().is_empty());
        spawner.forget(id);
        spawner.stop(id, Duration::ZERO);
    }

    #[test]
    fn each_shell_has_a_number_of_its_own() {
        let supervisor = Supervisor::new();
        let spawner = Spawner::new(supervisor.clone());
        let first = spawner.register(std::process::id(), "zsh");
        let second = spawner.register(std::process::id(), "bash");
        assert_ne!(first, second);
        spawner.forget(first);
        assert_eq!(supervisor.running().len(), 1);
        spawner.forget(second);
    }
}
