//! Short-lived programs such as `xcrun --find`, `cargo build` or `ps`, run through the supervisor with the login
//! shell's environment and a time limit.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use ide_lsp::Cancel;
use ide_process::{Ending, Kind, Supervisor};

/// What a helper printed and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Output {
    /// `None` when it was ended for taking too long or being cancelled.
    pub(crate) ending: Option<Ending>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Output {
    pub(crate) fn succeeded(&self) -> bool {
        self.ending == Some(Ending::Code(0))
    }
}

/// How a helper is run.
pub(crate) struct Helper<'a> {
    pub(crate) supervisor: &'a Supervisor,
    pub(crate) env: &'a [(String, String)],
    pub(crate) timeout: Duration,
    pub(crate) cancel: Option<&'a Cancel>,
}

/// How often a running helper looks whether it was cancelled.
const POLL: Duration = Duration::from_millis(100);

impl Helper<'_> {
    /// Runs `program` with `args` in `dir`; `line` hears each line of standard error as it comes.
    pub(crate) fn run(
        &self,
        program: &Path,
        args: &[&str],
        dir: &Path,
        line: &mut dyn FnMut(&str),
    ) -> std::io::Result<Output> {
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
            .map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into_owned());
        let spawned = self.supervisor.spawn(command, Kind::Tool, label)?;
        let stdout = spawned.stdout.map(|out| {
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = BufReader::new(out).read_to_string(&mut text);
                text
            })
        });
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let stderr = spawned.stderr.map(|err| {
            std::thread::spawn(move || {
                for text in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = tx.send(text);
                }
            })
        });
        let mut collected = String::new();
        let mut waited = Duration::ZERO;
        let ending = loop {
            while let Ok(text) = rx.try_recv() {
                line(&text);
                collected.push_str(&text);
                collected.push('\n');
            }
            if let Some(ending) = self.supervisor.wait(spawned.id, POLL) {
                break Some(ending);
            }
            waited += POLL;
            if waited >= self.timeout || self.cancel.is_some_and(Cancel::is_cancelled) {
                self.supervisor.stop(spawned.id, Duration::from_secs(1));
                break None;
            }
        };
        if let Some(stderr) = stderr {
            let _ = stderr.join();
        }
        for text in rx.try_iter() {
            line(&text);
            collected.push_str(&text);
            collected.push('\n');
        }
        let stdout = stdout.and_then(|t| t.join().ok()).unwrap_or_default();
        Ok(Output {
            ending,
            stdout,
            stderr: collected,
        })
    }
}

/// The value of `name` in `env`.
pub(crate) fn var<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// `name` on `path`, the PATH variable's value.
pub(crate) fn which(name: &str, path: Option<&str>) -> Option<PathBuf> {
    path?
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| is_executable(candidate))
}

pub(crate) fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
