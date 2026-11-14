//! Running a test tool for the Tests panel: through the supervisor as a process of the kind `Test`, which the activity
//! view shows and can stop, with the login shell's environment, in a trusted folder only (E22). Output is gathered as
//! it comes; a run is read again while it goes, on a worker, each time only what came since.

use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_core::LockIgnoringPoison;
use ide_process::{Ending, Kind, ProcessId, Supervisor};
use ide_test::{Reading, Report, Test, Tool};

/// A started run: its process and what it printed so far.
pub struct Running {
    /// The panel's name for the run.
    pub key: u64,
    pub id: ProcessId,
    pub output: Arc<Mutex<String>>,
    pub results: Option<PathBuf>,
    /// It lists tests rather than running them.
    pub listing: bool,
    /// How far the run was read, while no poll has it on the worker.
    pub progress: Option<Progress>,
    /// Every report of the run so far, by its place in the run's reading.
    pub reports: Vec<Option<Report>>,
    /// The board's revision of the run's tool right after the run's reports were last applied.
    pub applied: Option<u64>,
}

/// How far a run's output or results file was read.
#[derive(Default)]
pub struct Progress {
    reading: Option<Reading>,
    read: usize,
}

/// What a poll takes to the worker for one run.
pub struct Look {
    key: u64,
    id: ProcessId,
    tool: Tool,
    root: PathBuf,
    listing: bool,
    output: Arc<Mutex<String>>,
    results: Option<PathBuf>,
    progress: Progress,
}

/// What the worker found of a run.
pub struct Polled {
    pub key: u64,
    /// How the run ended; `None` while it goes.
    pub ending: Option<Ending>,
    /// The tests an ended listing named.
    pub tests: Option<Vec<Test>>,
    /// The results file was written anew, so the reports of the run start over.
    pub restarted: bool,
    /// The reports that changed, by their place in the run's reading.
    pub reports: Vec<(usize, Report)>,
    /// All the run printed, once it ended.
    pub output: Option<String>,
    pub progress: Progress,
}

/// Starts `command` with `env`, the login shell's environment, after writing the files it needs; `label` names it in
/// the activity view. It writes files and starts a process, so it runs off the main thread.
pub fn start(
    key: u64,
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
        key,
        id: spawned.id,
        output,
        results: command.results.clone(),
        listing,
        progress: Some(Progress::default()),
        reports: Vec::new(),
        applied: None,
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

impl Running {
    /// What the worker needs to read the run of `tool` in `root` further; `None` while a poll has it.
    pub fn look(&mut self, tool: Tool, root: &Path) -> Option<Look> {
        Some(Look {
            key: self.key,
            id: self.id,
            tool,
            root: root.to_path_buf(),
            listing: self.listing,
            output: self.output.clone(),
            results: self.results.clone(),
            progress: self.progress.take()?,
        })
    }
}

/// Reads a run further, on a worker: whether it ended, and what it said since the last look; a listing once it ended.
pub fn look(look: Look) -> Polled {
    let Look {
        key,
        id,
        tool,
        root,
        listing,
        output,
        results,
        mut progress,
    } = look;
    let ending = Supervisor::global().wait(id, Duration::ZERO);
    let done = ending.is_some();
    let mut polled = Polled {
        key,
        ending,
        tests: None,
        restarted: false,
        reports: Vec::new(),
        output: None,
        progress: Progress::default(),
    };
    if listing {
        if done {
            let said = match &results {
                Some(file) => std::fs::read_to_string(file).unwrap_or_default(),
                None => output.lock_ignoring_poison().clone(),
            };
            polled.tests = Some(tool.tests(&root, &said));
        }
    } else {
        let new = match &results {
            Some(file) => from_file(file, &mut progress, done, &mut polled.restarted),
            None => from_output(&output, &mut progress),
        };
        let reading = progress.reading.get_or_insert_with(|| Reading::new(tool, &root));
        reading.feed(&new);
        if done {
            reading.end();
        }
        polled.reports = reading.changed();
    }
    if done {
        polled.output = Some(output.lock_ignoring_poison().clone());
    }
    polled.progress = progress;
    polled
}

fn from_output(output: &Mutex<String>, progress: &mut Progress) -> String {
    let output = output.lock_ignoring_poison();
    let new = output.get(progress.read..).unwrap_or_default().to_string();
    progress.read = output.len();
    new
}

/// What the results file holds past what was read: whole lines only while the run goes on, as the tool may be in the
/// middle of one. A file shorter than what was read was written anew, so its reading starts over.
fn from_file(file: &Path, progress: &mut Progress, done: bool, restarted: &mut bool) -> String {
    let Ok(mut opened) = std::fs::File::open(file) else {
        return String::new();
    };
    let length = opened.metadata().map_or(0, |m| m.len());
    if length < progress.read as u64 {
        *progress = Progress::default();
        *restarted = true;
    }
    let mut bytes = Vec::new();
    if opened.seek(std::io::SeekFrom::Start(progress.read as u64)).is_err() || opened.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    if !done {
        let whole = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |end| end + 1);
        bytes.truncate(whole);
    }
    progress.read += bytes.len();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Stops the run.
pub fn stop(running: &Running) {
    Supervisor::global().stop(running.id, Duration::from_secs(2));
}
