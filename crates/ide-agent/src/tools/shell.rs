//! Running a command line in the user's shell, in the project, under the supervisor with the login shell's
//! environment; with the thread's sandbox on, inside `sandbox-exec` with writes limited to the project and no network.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_process::tail::{self, Tail};
use ide_process::{Ending, Kind};
use ide_thread::ToolOutput;
use serde_json::Value;

use super::{Context, Outcome, number, required};
use crate::notes::ToolNote;

/// How long a command may run when the call names no limit.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// The longest a call may allow.
const MAX_TIMEOUT: Duration = Duration::from_secs(600);
/// The end of each stream that is kept.
const KEPT: usize = 32 * 1024;
/// How long the output may stay open after the shell ended, as a program it left behind keeps it.
const AFTERMATH: Duration = Duration::from_secs(2);

/// The program that runs the command line: the login shell, inside the sandbox when it is on.
fn command(context: &Context<'_>, line: &str) -> Command {
    let shell = context
        .workspace
        .environment()
        .iter()
        .find(|(name, _)| name == "SHELL")
        .map_or_else(ide_process::shell_env::user_shell, |(_, value)| PathBuf::from(value));
    let mut command = if context.sandbox {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command.arg("-p").arg(context.workspace.sandbox_profile()).arg(&shell);
        command
    } else {
        Command::new(&shell)
    };
    command
        .arg("-c")
        .arg(line)
        .env_clear()
        .envs(context.workspace.environment().iter().map(|(k, v)| (k, v)))
        .current_dir(context.workspace.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub(super) fn run(input: &Value, context: &Context<'_>) -> Result<Outcome, ToolNote> {
    let line = required(input, "command")?;
    let timeout = number(input, "timeout_seconds")
        .map_or(DEFAULT_TIMEOUT, Duration::from_secs)
        .min(MAX_TIMEOUT);
    let supervisor = context.workspace.supervisor();
    let label: String = line.chars().take(60).collect();
    let spawned = supervisor
        .spawn(command(context, line), Kind::Tool, label)
        .map_err(|e| ToolNote::NotStarted(e.to_string()))?;
    let (out, err) = (
        Arc::new(Mutex::new(Tail::new(KEPT))),
        Arc::new(Mutex::new(Tail::new(KEPT))),
    );
    let readers: Vec<_> = [
        spawned.stdout.map(|pipe| tail::collect(pipe, out.clone())),
        spawned.stderr.map(|pipe| tail::collect(pipe, err.clone())),
    ]
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .collect();
    let deadline = Instant::now() + timeout;
    let mut stopped: Option<&str> = None;
    let ending = loop {
        if let Some(ending) = supervisor.wait(spawned.id, Duration::from_millis(100)) {
            break ending;
        }
        if context.cancel.is_cancelled() || Instant::now() >= deadline {
            stopped = Some(if context.cancel.is_cancelled() {
                "stopped by the user"
            } else {
                "stopped at its time limit"
            });
            supervisor.stop(spawned.id, Duration::from_millis(500));
            break supervisor
                .wait(spawned.id, Duration::from_secs(3))
                .unwrap_or(Ending::Unknown);
        }
    };
    let aftermath = Instant::now() + AFTERMATH;
    while readers.iter().any(|r| !r.is_finished()) && Instant::now() < aftermath {
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = |tail: &Arc<Mutex<Tail>>| {
        let tail = tail.lock().unwrap_or_else(|e| e.into_inner());
        let dropped = tail.dropped();
        let text = tail.text();
        if dropped > 0 {
            format!("[… {dropped} earlier bytes left out]\n{text}")
        } else {
            text
        }
    };
    let (stdout, stderr) = (text(&out), text(&err));
    let status = match ending {
        Ending::Code(code) => format!("exit code {code}"),
        Ending::Signal(signal) => format!("ended by signal {signal}"),
        Ending::Unknown => "ended".to_string(),
    };
    let mut content = match stopped {
        Some(why) => format!("{status}, {why}"),
        None => status,
    };
    for (name, stream) in [("stdout", &stdout), ("stderr", &stderr)] {
        if !stream.trim().is_empty() {
            content.push_str(&format!("\n<{name}>\n{}\n</{name}>", stream.trim_end()));
        }
    }
    Ok(Outcome {
        content,
        is_error: !matches!(ending, Ending::Code(0)) || stopped.is_some(),
        output: Some(ToolOutput::Terminal {
            stdout,
            stderr,
            interrupted: stopped.is_some(),
        }),
        ..Outcome::default()
    })
}
