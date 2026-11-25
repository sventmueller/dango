//! Running a command line in the user's shell, in the project, under the supervisor with the login shell's
//! environment; with the thread's sandbox on, inside `sandbox-exec` with writes limited to the project and no network.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ide_core::Cancel;
use ide_process::pipes::{Pipes, Stream};
use ide_process::tail::Tail;
use ide_process::{Ending, Kind, ProcessId, Supervisor};
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
/// How often a running command is looked at for being cancelled or past its time.
const POLL: Duration = Duration::from_millis(100);
/// How long a stopped command may take to end before its end counts as unknown.
const STOPPING: Duration = Duration::from_secs(3);

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
        .spawn(command(context, line), Kind::AgentCommand, label)
        .map_err(|e| ToolNote::NotStarted(e.to_string()))?;
    let pipes = Pipes::new(None, None, spawned.stdout, spawned.stderr).ok();
    let (mut out, mut err) = (Tail::new(KEPT), Tail::new(KEPT));
    let (ending, stopped) = serve(
        supervisor,
        spawned.id,
        pipes,
        Instant::now() + timeout,
        context.cancel,
        &mut |stream, bytes| match stream {
            Stream::Stdout => out.push(bytes),
            Stream::Stderr => err.push(bytes),
        },
    );
    let text = |tail: &Tail| {
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

/// Serves the pipes of the command `id` on this thread until it ended and its pipes closed, or [`AFTERMATH`] after its
/// end, as a program it left behind may hold them open; stops it when it is cancelled or past `deadline`, and says why.
fn serve(
    supervisor: &Supervisor,
    id: ProcessId,
    mut pipes: Option<Pipes>,
    deadline: Instant,
    cancel: &Cancel,
    arrived: &mut dyn FnMut(Stream, &[u8]),
) -> (Ending, Option<&'static str>) {
    let mut stopped: Option<(&'static str, Instant)> = None;
    let mut ended: Option<(Ending, Instant)> = None;
    loop {
        let open = pipes.as_ref().is_some_and(Pipes::open);
        let now = Instant::now();
        let look = match ended {
            Some((ending, until)) => {
                if !open || now >= until {
                    return (ending, stopped.map(|(why, _)| why));
                }
                until - now
            }
            None => {
                if stopped.is_none() && (cancel.is_cancelled() || now >= deadline) {
                    let why = if cancel.is_cancelled() {
                        "stopped by the user"
                    } else {
                        "stopped at its time limit"
                    };
                    supervisor.stop(id, Duration::from_millis(500));
                    stopped = Some((why, now + STOPPING));
                }
                let ending = supervisor.wait(id, if open { Duration::ZERO } else { POLL });
                let ending = ending.or_else(|| {
                    stopped
                        .is_some_and(|(_, until)| Instant::now() >= until)
                        .then_some(Ending::Unknown)
                });
                if let Some(ending) = ending {
                    ended = Some((ending, Instant::now() + AFTERMATH));
                    continue;
                }
                POLL
            }
        };
        if open
            && let Some(served) = pipes.as_mut()
            && served.step(Some(look), arrived).is_err()
        {
            pipes = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::http::Http;
    use crate::workspace::Workspace;

    fn run_in(name: &str, input: &Value, cancel: &Cancel) -> Outcome {
        let dir = ide_core::testing::scratch(&format!("agent-shell-{name}"));
        let environment = vec![
            ("SHELL".to_string(), "/bin/sh".to_string()),
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ];
        let workspace = Workspace::new(dir.path(), environment, Supervisor::new(), Http::new()).unwrap();
        let context = Context {
            workspace: &workspace,
            sandbox: false,
            cancel,
            checked: None,
        };
        run(input, &context).unwrap()
    }

    fn streams(outcome: &Outcome) -> (String, String, bool) {
        match &outcome.output {
            Some(ToolOutput::Terminal {
                stdout,
                stderr,
                interrupted,
            }) => (stdout.clone(), stderr.clone(), *interrupted),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn both_streams_and_the_exit_code_arrive_apart() {
        let outcome = run_in(
            "streams",
            &json!({"command": "echo out; echo err >&2; exit 3"}),
            &Cancel::new(),
        );
        assert_eq!(streams(&outcome), ("out\n".to_string(), "err\n".to_string(), false));
        assert!(outcome.is_error);
        assert!(outcome.content.starts_with("exit code 3"), "{}", outcome.content);
    }

    #[test]
    fn output_past_what_is_kept_keeps_its_end_and_counts_the_rest() {
        let outcome = run_in(
            "long",
            &json!({"command": "yes a | head -c 100000; echo end"}),
            &Cancel::new(),
        );
        let (stdout, _, _) = streams(&outcome);
        assert!(
            stdout.starts_with("[… "),
            "{}",
            stdout.chars().take(40).collect::<String>()
        );
        assert!(
            stdout.ends_with("a\nend\n"),
            "{:?}",
            &stdout[stdout.len().saturating_sub(80)..]
        );
        assert!(!outcome.is_error);
    }

    #[test]
    fn a_program_left_behind_holding_the_output_does_not_hold_the_call() {
        let started = Instant::now();
        let outcome = run_in(
            "left-behind",
            &json!({"command": "sleep 10 & echo done"}),
            &Cancel::new(),
        );
        assert!(
            started.elapsed() < AFTERMATH + Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(streams(&outcome).0, "done\n");
        assert!(!outcome.is_error);
    }

    #[test]
    fn a_command_past_its_time_limit_is_stopped_with_what_it_printed() {
        let started = Instant::now();
        let outcome = run_in(
            "limit",
            &json!({"command": "echo before; exec sleep 30", "timeout_seconds": 1}),
            &Cancel::new(),
        );
        assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
        assert_eq!(streams(&outcome), ("before\n".to_string(), String::new(), true));
        assert!(outcome.is_error);
        assert!(
            outcome.content.contains("stopped at its time limit"),
            "{}",
            outcome.content
        );
    }

    #[test]
    fn a_cancelled_command_is_stopped() {
        let cancel = Cancel::new();
        cancel.cancel();
        let outcome = run_in("cancel", &json!({"command": "exec sleep 30"}), &cancel);
        assert!(streams(&outcome).2);
        assert!(outcome.content.contains("stopped by the user"), "{}", outcome.content);
    }
}
