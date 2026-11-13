//! `claude -p` as the writer of commit messages when no model is chosen (E88): a small model, no tools, none of the
//! user's or a project's customizations, in an empty folder of its own, so that the checked request is all it gets
//! besides the fixed instruction. It signs in through `claude` itself (E21).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_agent::Cancel;
use ide_process::{Kind, Supervisor};
use serde_json::Value;
use splat_i18n::{t, tf};

use super::source::{Failure, SYSTEM};

/// The small model `claude` knows by this alias.
pub(crate) const MODEL: &str = "haiku";
/// How long a stopped `claude` gets before it is killed.
const STOP_GRACE: Duration = Duration::from_millis(500);
/// How often a running request looks at its cancel.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// What one line of `claude -p --output-format stream-json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A piece of the answer.
    Text(String),
    /// The end, with the whole answer.
    Done(String),
    /// The end, with what went wrong.
    Failed(String),
}

/// The arguments: print mode, a small model, no tools, no customizations, no session left behind, the answer as it
/// streams.
fn arguments() -> Vec<&'static str> {
    vec![
        "-p",
        "--model",
        MODEL,
        "--tools",
        "",
        "--safe-mode",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--system-prompt",
        SYSTEM,
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--verbose",
    ]
}

/// What `line` of the output says, if anything this module uses.
pub fn parse(line: &str) -> Option<Event> {
    let value: Value = serde_json::from_str(line).ok()?;
    match value["type"].as_str()? {
        "stream_event" => {
            let event = &value["event"];
            let delta = &event["delta"];
            (event["type"] == "content_block_delta" && delta["type"] == "text_delta")
                .then(|| Event::Text(delta["text"].as_str().unwrap_or("").to_string()))
        }
        "result" => {
            let text = value["result"].as_str().unwrap_or("").to_string();
            let failed = value["is_error"].as_bool().unwrap_or(false) || value["subtype"] != "success";
            Some(if failed { Event::Failed(text) } else { Event::Done(text) })
        }
        _ => None,
    }
}

/// Runs `program` on `text` with `environment` and hands each piece of the answer to `on`.
pub fn run(
    program: &Path,
    environment: &[(String, String)],
    text: &str,
    cancel: &Cancel,
    on: &mut dyn FnMut(&str),
) -> Result<String, Failure> {
    let folder = empty_folder().map_err(|e| Failure::Failed(e.to_string()))?;
    let result = run_in(program, environment, &folder, text, cancel, on);
    let _ = std::fs::remove_dir_all(&folder);
    result
}

fn run_in(
    program: &Path,
    environment: &[(String, String)],
    folder: &Path,
    text: &str,
    cancel: &Cancel,
    on: &mut dyn FnMut(&str),
) -> Result<String, Failure> {
    let mut command = Command::new(program);
    command
        .args(arguments())
        .current_dir(folder)
        .env_clear()
        .envs(environment.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let supervisor = Supervisor::global();
    let spawned = supervisor
        .spawn(command, Kind::Agent, "claude -p (commit message)")
        .map_err(|e| Failure::Failed(tf("claude ließ sich nicht starten: {}", &[&e.to_string()])))?;
    let id = spawned.id;
    if let Some(mut stdin) = spawned.stdin {
        let input = text.as_bytes().to_vec();
        let _ = std::thread::Builder::new()
            .name("claude-message-stdin".into())
            .spawn(move || drop(stdin.write_all(&input)));
    }
    let said = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = spawned.stderr {
        let said = said.clone();
        let _ = std::thread::Builder::new()
            .name("claude-message-stderr".into())
            .spawn(move || {
                let mut text = String::new();
                let _ = stderr.read_to_string(&mut text);
                *said.lock().unwrap_or_else(|e| e.into_inner()) = text;
            });
    }
    let finished = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    {
        let (cancel, finished, stopped) = (cancel.clone(), finished.clone(), stopped.clone());
        let _ = std::thread::Builder::new()
            .name("claude-message-cancel".into())
            .spawn(move || {
                while !finished.load(Ordering::SeqCst) {
                    if cancel.is_cancelled() {
                        stopped.store(true, Ordering::SeqCst);
                        Supervisor::global().stop(id, STOP_GRACE);
                        return;
                    }
                    std::thread::sleep(CANCEL_POLL);
                }
            });
    }
    let mut outcome: Option<Result<String, Failure>> = None;
    let mut received = String::new();
    if let Some(stdout) = spawned.stdout {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            match parse(&line) {
                Some(Event::Text(piece)) => {
                    received.push_str(&piece);
                    on(&piece);
                }
                Some(Event::Done(text)) => outcome = Some(Ok(if text.is_empty() { received.clone() } else { text })),
                Some(Event::Failed(text)) => outcome = Some(Err(Failure::Failed(text))),
                None => {}
            }
        }
    }
    let _ = supervisor.wait(id, Duration::from_secs(10));
    finished.store(true, Ordering::SeqCst);
    if stopped.load(Ordering::SeqCst) {
        return Err(Failure::Cancelled);
    }
    outcome.unwrap_or_else(|| {
        let said = said.lock().unwrap_or_else(|e| e.into_inner()).trim().to_string();
        Err(Failure::Failed(if said.is_empty() {
            t("claude hat ohne Antwort aufgehört.").to_string()
        } else {
            said
        }))
    })
}

/// A new empty folder in the temporary folder, where `claude` finds no project of the user's.
fn empty_folder() -> std::io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let folder = std::env::temp_dir().join(format!("ide-commit-message-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder)?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_gives_pieces_of_text_and_its_end() {
        let piece = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Fix "}}}"#;
        assert_eq!(parse(piece), Some(Event::Text("Fix ".into())));
        let start = r#"{"type":"stream_event","event":{"type":"message_start","message":{}}}"#;
        assert_eq!(parse(start), None);
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"Fix the parser"}"#;
        assert_eq!(parse(done), Some(Event::Done("Fix the parser".into())));
        let failed =
            r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}"#;
        assert_eq!(
            parse(failed),
            Some(Event::Failed("Not logged in · Please run /login".into()))
        );
        assert_eq!(parse(r#"{"type":"system","subtype":"init"}"#), None);
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn claude_gets_no_tools_and_none_of_the_user_s_customizations() {
        let args = arguments();
        let after = |flag: &str| args.iter().position(|a| *a == flag).map(|i| args[i + 1]);
        assert_eq!(after("--tools"), Some(""));
        assert_eq!(after("--model"), Some(MODEL));
        assert_eq!(after("--system-prompt"), Some(SYSTEM));
        assert!(args.contains(&"--safe-mode") && args.contains(&"--no-session-persistence"));
    }

    const FAKE: &str = r#"#!/bin/sh
piece() { printf '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"%s"}}}\n' "$1"; }
case "$FAKE" in
answer)
    input=$(cat)
    printf '{"type":"system","subtype":"init"}\n'
    piece "Fix "
    piece "parser"
    printf '{"type":"result","subtype":"success","is_error":false,"result":"got %s in %s with %s arguments"}\n' "$input" "$(pwd)" "$#"
    ;;
pieces)
    piece "Fix "
    piece "parser"
    printf '{"type":"result","subtype":"success","is_error":false,"result":""}\n'
    ;;
refused)
    printf '{"type":"result","subtype":"success","is_error":true,"result":"Not logged in"}\n'
    ;;
stderr)
    echo "  boom  " >&2
    exit 1
    ;;
slow)
    piece "Fix "
    exec sleep 30
    ;;
esac
"#;

    /// A stand-in for `claude` that answers as `mode` says.
    fn fake(mode: &str) -> (PathBuf, Vec<(String, String)>) {
        use std::os::unix::fs::PermissionsExt;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ide-fake-claude-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("claude");
        std::fs::write(&program, FAKE).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let environment = vec![
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("FAKE".to_string(), mode.to_string()),
        ];
        (program, environment)
    }

    fn ask(mode: &str, cancel_on_first_piece: bool) -> (Result<String, Failure>, Vec<String>) {
        let (program, environment) = fake(mode);
        let cancel = Cancel::new();
        let mut pieces = Vec::new();
        let result = run(&program, &environment, "the diff", &cancel, &mut |piece| {
            pieces.push(piece.to_string());
            if cancel_on_first_piece {
                cancel.cancel();
            }
        });
        let _ = std::fs::remove_dir_all(program.parent().unwrap());
        (result, pieces)
    }

    #[test]
    fn claude_reads_the_request_in_an_empty_folder_that_is_gone_afterwards() {
        let (result, pieces) = ask("answer", false);
        assert_eq!(pieces, ["Fix ", "parser"]);
        let answer = result.unwrap();
        let folder = answer
            .strip_prefix("got the diff in ")
            .and_then(|rest| rest.strip_suffix(&format!(" with {} arguments", arguments().len())))
            .unwrap_or_else(|| panic!("{answer}"));
        let folder = Path::new(folder);
        assert!(
            folder
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ide-commit-message-"),
            "{answer}"
        );
        assert!(!folder.exists(), "{answer}");
    }

    #[test]
    fn an_end_without_text_gives_the_pieces_that_streamed() {
        let (result, _) = ask("pieces", false);
        assert_eq!(result, Ok("Fix parser".to_string()));
    }

    #[test]
    fn a_failed_end_or_a_silent_exit_says_what_claude_said() {
        assert_eq!(ask("refused", false).0, Err(Failure::Failed("Not logged in".into())));
        assert_eq!(ask("stderr", false).0, Err(Failure::Failed("boom".into())));
        assert_eq!(
            ask("nothing", false).0,
            Err(Failure::Failed(t("claude hat ohne Antwort aufgehört.").into()))
        );
    }

    #[test]
    fn a_cancel_stops_claude_while_it_answers() {
        let started = std::time::Instant::now();
        let (result, pieces) = ask("slow", true);
        assert_eq!(result, Err(Failure::Cancelled));
        assert_eq!(pieces, ["Fix "]);
        assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    }

    #[test]
    fn a_program_that_does_not_start_says_so() {
        let missing = std::env::temp_dir().join("ide-no-such-claude/claude");
        let result = run(&missing, &[], "", &Cancel::new(), &mut |_| {});
        let prefix = tf("claude ließ sich nicht starten: {}", &[""]);
        assert!(
            matches!(&result, Err(Failure::Failed(why)) if why.starts_with(&prefix) && why.len() > prefix.len()),
            "{result:?}"
        );
    }
}
