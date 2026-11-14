//! Running a test tool for the Tests panel: through the supervisor as a process of the kind `Test`, which the activity
//! view shows and can stop, with the login shell's environment, in a trusted folder only (E22). Output is gathered as
//! it comes; tools that report each test as it starts are read again while they run.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_core::LockIgnoringPoison;
use ide_process::{Ending, Kind, ProcessId, Supervisor};

/// A started run: its process and what it printed so far.
pub struct Running {
    pub id: ProcessId,
    pub output: Arc<Mutex<String>>,
    pub results: Option<PathBuf>,
    /// It lists tests rather than running them.
    pub listing: bool,
}

/// Starts `command` with `env`, the login shell's environment, after writing the files it needs; `label` names it in
/// the activity view.
pub fn start(
    command: &ide_test::Command,
    env: &[(String, String)],
    label: &str,
    listing: bool,
) -> std::io::Result<Running> {
    for (path, content) in &command.files {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, content)?;
    }
    let program = resolve(&command.program, env);
    let mut process = Command::new(program);
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .envs(command.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut spawned = Supervisor::global().spawn(process, Kind::Test, label)?;
    let output = Arc::new(Mutex::new(String::new()));
    for pipe in [
        spawned.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>),
        spawned.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let output = output.clone();
        crate::util::spawn("test-output", move || gather(pipe, &output)).map_err(|why| std::io::Error::other(why.0))?;
    }
    Ok(Running {
        id: spawned.id,
        output,
        results: command.results.clone(),
        listing,
    })
}

fn gather(mut pipe: Box<dyn Read + Send>, output: &Mutex<String>) {
    let mut buffer = [0u8; 8192];
    while let Ok(n) = pipe.read(&mut buffer) {
        if n == 0 {
            break;
        }
        output
            .lock_ignoring_poison()
            .push_str(&String::from_utf8_lossy(&buffer[..n]));
    }
}

/// `program` as found on the login shell's PATH, or as it is where it is a path or not found.
fn resolve(program: &str, env: &[(String, String)]) -> PathBuf {
    if program.contains('/') {
        return PathBuf::from(program);
    }
    env.iter()
        .find(|(k, _)| k == "PATH")
        .and_then(|(_, path)| {
            path.split(':')
                .map(|dir| Path::new(dir).join(program))
                .find(|candidate| candidate.is_file())
        })
        .unwrap_or_else(|| PathBuf::from(program))
}

/// How the run ended, waiting up to `wait`; `None` while it runs.
pub fn ended(running: &Running, wait: Duration) -> Option<Ending> {
    Supervisor::global().wait(running.id, wait)
}

/// What the run said so far: its results file where it has one, else its output.
pub fn said(running: &Running) -> String {
    match &running.results {
        Some(file) => std::fs::read_to_string(file).unwrap_or_default(),
        None => running.output.lock_ignoring_poison().clone(),
    }
}

/// Stops the run.
pub fn stop(running: &Running) {
    Supervisor::global().stop(running.id, Duration::from_secs(2));
}
