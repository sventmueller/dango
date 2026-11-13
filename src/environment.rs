//! The environment processes get: the user's login shell's, read once in the background at launch.

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use ide_process::Supervisor;
use ide_process::shell_env::{self, CaptureError};

/// How long a slow shell startup may take before the app falls back to its own environment.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// Where launchd's PATH lacks the usual places for developer tools on a Mac.
const FALLBACK_PATHS: [&str; 6] = [
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
];

static ENVIRONMENT: OnceLock<Vec<(String, String)>> = OnceLock::new();
static READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Reads the login shell's environment on a thread of its own.
pub fn start() {
    let started = crate::util::spawn("login-env", || {
        let home = ide_core::paths::home();
        let environment =
            match shell_env::capture(Supervisor::global(), &shell_env::user_shell(), &home, CAPTURE_TIMEOUT) {
                Ok(environment) => environment,
                Err(error) => {
                    crate::diag::report("login shell environment", describe(&error));
                    fallback()
                }
            };
        publish(environment);
    });
    if let Err(why) = started {
        crate::diag::report("login shell environment", why);
        publish(fallback());
    }
}

fn publish(environment: Vec<(String, String)>) {
    let _ = ENVIRONMENT.set(environment);
    let (lock, ready) = &READY;
    *lock.lock_ignoring_poison() = true;
    ready.notify_all();
}

/// The environment, once it is known.
pub fn get() -> Option<&'static [(String, String)]> {
    ENVIRONMENT.get().map(Vec::as_slice)
}

/// The environment, waiting up to `timeout` for it.
pub fn wait(timeout: Duration) -> Option<&'static [(String, String)]> {
    let (lock, ready) = &READY;
    let guard = lock.lock_ignoring_poison();
    let _ = ready
        .wait_timeout_while(guard, timeout, |done| !*done)
        .ignoring_poison();
    get()
}

fn describe(error: &CaptureError) -> String {
    match error {
        CaptureError::Start(e) => format!("could not start the shell: {e}"),
        CaptureError::Timeout => "the shell did not finish in time".into(),
        CaptureError::NoEnvironment => "the shell printed no environment".into(),
    }
}

/// The app's own environment with the usual tool folders on its PATH.
fn fallback() -> Vec<(String, String)> {
    let mut environment: Vec<(String, String)> = std::env::vars().filter(|(name, _)| name != "PATH").collect();
    let home = ide_core::paths::home();
    let mut path: Vec<String> = [".cargo/bin", ".local/bin", "go/bin"]
        .iter()
        .map(|dir| home.join(dir).to_string_lossy().into_owned())
        .collect();
    path.extend(FALLBACK_PATHS.iter().map(ToString::to_string));
    environment.push(("PATH".into(), path.join(":")));
    environment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_capture_failure_is_worded_for_the_log() {
        let start = describe(&CaptureError::Start(std::io::Error::other("no such shell")));
        assert_eq!(start, "could not start the shell: no such shell");
        assert_eq!(describe(&CaptureError::Timeout), "the shell did not finish in time");
        assert_eq!(
            describe(&CaptureError::NoEnvironment),
            "the shell printed no environment"
        );
    }

    #[test]
    fn the_fallback_has_one_path_with_the_home_tool_folders_first_and_the_usual_places_after() {
        let environment = fallback();
        let paths: Vec<&str> = environment
            .iter()
            .filter(|(name, _)| name == "PATH")
            .map(|(_, value)| value.as_str())
            .collect();
        let [path] = paths.as_slice() else {
            panic!("one PATH expected, got {paths:?}");
        };
        let entries: Vec<&str> = path.split(':').collect();
        let home = ide_core::paths::home();
        let tools: Vec<String> = [".cargo/bin", ".local/bin", "go/bin"]
            .iter()
            .map(|dir| home.join(dir).to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries[..3], tools.iter().map(String::as_str).collect::<Vec<_>>()[..]);
        assert_eq!(entries[3..], FALLBACK_PATHS[..]);
        let others = std::env::vars().filter(|(name, _)| name != "PATH").count();
        assert_eq!(environment.len(), others + 1);
    }
}
