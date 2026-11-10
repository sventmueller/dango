//! Where git runs: one app-wide `Git` once the login shell's environment is known, a lane for local work and threads
//! for work that may take minutes.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ide_core::lane::Lane;
use ide_git::Git;
use ide_process::Supervisor;

use crate::util::main_soon;

/// Local git work, one job at a time, so that a status read after a change sees it.
static LOCAL: Lane = Lane::new("ide-git", 1, 64);

static GIT: OnceLock<Git> = OnceLock::new();

/// How long the first job waits for the login shell's environment before it gives up for this time.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// The app's git; waits for the login shell's environment, so only for worker threads. `None` while it is not known.
pub fn git() -> Option<Git> {
    if let Some(git) = GIT.get() {
        return Some(git.clone());
    }
    let environment = crate::environment::wait(ENVIRONMENT_WAIT)?;
    let environment = for_selftest(environment.to_vec());
    Some(GIT.get_or_init(|| Git::new(Supervisor::global(), environment)).clone())
}

/// Keeps the user's own git configuration, such as signing or hooks paths, out of a self-test.
#[cfg(feature = "selftest")]
fn for_selftest(mut environment: Vec<(String, String)>) -> Vec<(String, String)> {
    if std::env::var_os("IDE_SELFTEST").is_some() {
        environment.push(("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()));
        environment.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    }
    environment
}

#[cfg(not(feature = "selftest"))]
fn for_selftest(environment: Vec<(String, String)>) -> Vec<(String, String)> {
    environment
}

/// Runs `work` on the git lane and hands its result to `done` on the main thread, `None` if it panicked; false when
/// the lane is full and nothing runs.
pub fn local<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + Send + 'static,
) -> bool {
    LOCAL.ask(work, move |result| main_soon(move || done(result))).is_ok()
}

/// Runs `work` on a thread of its own, for hooks and the network, which may take minutes, and hands its result to
/// `done` on the main thread; `None` if it panicked or no thread could start.
pub fn long<T: Send + 'static, D: FnOnce(Option<T>) + Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: D,
) {
    let slot = Arc::new(Mutex::new(Some(done)));
    let theirs = slot.clone();
    let spawned = std::thread::Builder::new().name("ide-git-long".into()).spawn(move || {
        let result = ide_core::lane::contained(work);
        if let Some(done) = theirs.lock().unwrap_or_else(|e| e.into_inner()).take() {
            main_soon(move || done(result));
        }
    });
    if let Err(error) = spawned {
        crate::diag::report("starting a git thread", error);
        if let Some(done) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            main_soon(move || done(None));
        }
    }
}
