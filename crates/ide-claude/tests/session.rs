//! The session against a stand-in for `claude`: a shell script that waits for `initialize`, prints canned frames
//! and keeps everything the session writes. One ignored test runs the real CLI.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ide_claude::{ControlError, Event, Launch, PermissionAnswer, RemoteControl, Session, SessionError};
use ide_process::Supervisor;
use ide_thread::{Decision, EntryKind, Lane, RequestState, Status, Thread, ThreadId, UserMessage};
use serde_json::{Value, json};

const WAIT: Duration = Duration::from_secs(10);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-claude-session-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A stand-in CLI in `dir` that answers `initialize` with `account` and then prints `frames`.
fn stand_in(dir: &Path, account: Value, frames: &[Value]) -> Launch {
    let answer = json!({"commands": [], "models": [{"value": "haiku", "displayName": "Haiku"}], "account": account, "output_style": "default"});
    stand_in_answering(dir, answer, frames)
}

/// A stand-in CLI in `dir` that answers `initialize` with `answer` and then prints `frames`.
fn stand_in_answering(dir: &Path, answer: Value, frames: &[Value]) -> Launch {
    let mut lines = vec![
        json!({"type": "control_response", "response": {"subtype": "success", "request_id": "req-1", "response": answer}}),
    ];
    lines.extend_from_slice(frames);
    let fixture: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join("fixture.ndjson"), fixture).unwrap();
    let script = dir.join("claude");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$OUT.args\"\nread -r first\nprintf '%s\\n' \"$first\" > \"$OUT\"\ncat \"$FIXTURE\"\n[ -n \"$THEN_EXIT\" ] && exit 0\ncat >> \"$OUT\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        (
            "FIXTURE".to_string(),
            dir.join("fixture.ndjson").to_string_lossy().into_owned(),
        ),
        (
            "OUT".to_string(),
            dir.join("written.ndjson").to_string_lossy().into_owned(),
        ),
    ];
    Launch::new(script, env, dir)
}

fn start(launch: &Launch, supervisor: &Supervisor) -> (Session, Receiver<Event>) {
    let (tx, rx) = channel();
    let thread = Thread::new(ThreadId::new(), "claude", launch.cwd.clone());
    let session = Session::start(supervisor, launch, thread, move |event| {
        let _ = tx.send(event);
    })
    .unwrap();
    (session, rx)
}

/// Waits for an event `want` accepts.
fn wait_for(rx: &Receiver<Event>, mut want: impl FnMut(&Event) -> bool) -> Event {
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = rx.recv_timeout(left).expect("the awaited event came in time");
        if want(&event) {
            return event;
        }
    }
}

fn written(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("written.ndjson"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_session_answers_permissions_refuses_unknown_requests_and_ends_cleanly() {
    let dir = scratch("flow");
    let launch = stand_in(
        &dir,
        json!({"subscriptionType": "pro", "apiProvider": "firstParty"}),
        &[
            json!({"type": "system", "subtype": "init", "session_id": "s-1", "cwd": "/w", "model": "claude-haiku-5-5", "permissionMode": "default", "tools": ["Bash"], "claude_code_version": "2.1.319", "capabilities": ["interrupt_receipt_v1"]}),
            json!({"type": "assistant", "message": {"id": "m1", "content": [{"type": "tool_use", "id": "toolu_x", "name": "Bash", "input": {"command": "ls"}}]}, "parent_tool_use_id": null}),
            json!({"type": "control_request", "request_id": "cli-1", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "toolu_x"}}),
            json!({"type": "control_request", "request_id": "cli-2", "request": {"subtype": "summon_dragon"}}),
            json!({"type": "assistant", "message": {"id": "m2", "content": [{"type": "text", "text": "Hello"}]}, "parent_tool_use_id": null}),
        ],
    );
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    wait_for(&rx, |_| session.with_thread(|t| !t.pending_requests().is_empty()));
    assert_eq!(session.with_thread(Thread::status), Status::RequiresAction);
    assert!(matches!(
        session.answer(
            "nope",
            PermissionAnswer::Deny {
                message: String::new(),
                interrupt: false
            }
        ),
        Err(SessionError::UnknownRequest(_))
    ));
    session
        .answer(
            "cli-1",
            PermissionAnswer::Allow {
                updated_input: Some(json!({"command": "ls -a"})),
                always: Vec::new(),
            },
        )
        .unwrap();
    session
        .send(UserMessage {
            text: "Hi there\nsecond line".into(),
            ..UserMessage::default()
        })
        .unwrap();
    let interrupt = session.interrupt();
    wait_for(&rx, |_| {
        session.with_thread(|t| {
            t.entries(&Lane::Main)
                .iter()
                .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown == "Hello"))
        })
    });
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
    assert!(session.is_ended());
    assert_eq!(interrupt.wait(), Err(ControlError::Closed));
    assert!(matches!(
        session.send(UserMessage::default()),
        Err(SessionError::Closed)
    ));

    let thread = session.snapshot();
    assert_eq!(thread.title(), "Hi there");
    assert_eq!(thread.status(), Status::Idle);
    assert_eq!(thread.session(), Some("s-1"));
    let Some(EntryKind::PermissionRequest(request)) = thread.entry_by_key("perm:cli-1").map(|e| &e.kind) else {
        panic!()
    };
    assert_eq!(
        request.state,
        RequestState::Decided(Decision::Allowed {
            always: false,
            edited: true
        })
    );
    let caps = session.capabilities();
    assert_eq!(caps.version.as_deref(), Some("2.1.319"));
    assert!(caps.interrupt_receipts());
    assert_eq!(caps.models[0].value, "haiku");

    let frames = written(&dir);
    let find = |pred: &dyn Fn(&Value) -> bool| frames.iter().find(|f| pred(f)).cloned();
    let init = find(&|f| f["request"]["subtype"] == "initialize").expect("initialize was sent first");
    assert_eq!(frames[0], init);
    assert_eq!(init["request"]["perTaskStopAffordance"], true);
    let allow = find(&|f| f["response"]["request_id"] == "cli-1").expect("the permission answer");
    assert_eq!(allow["response"]["response"]["updatedInput"]["command"], "ls -a");
    assert_eq!(allow["response"]["response"]["toolUseID"], "toolu_x");
    let refusal = find(&|f| f["response"]["request_id"] == "cli-2").expect("the refusal");
    assert_eq!(refusal["response"]["subtype"], "error");
    let prompt = find(&|f| f["type"] == "user").expect("the prompt");
    assert_eq!(prompt["message"]["content"][0]["text"], "Hi there\nsecond line");
    assert_eq!(prompt["origin"]["kind"], "human");
    assert!(find(&|f| f["request"]["subtype"] == "interrupt").is_some());
    let args = std::fs::read_to_string(dir.join("written.ndjson.args")).unwrap();
    assert!(args.lines().any(|a| a == "--permission-prompt-tool=stdio"));
    assert!(args.lines().any(|a| a == "--input-format=stream-json"));
}

#[test]
fn a_cli_without_sign_in_is_reported_so_the_user_can_log_in_in_a_terminal() {
    let dir = scratch("signed-out");
    let launch = stand_in(&dir, json!({"tokenSource": "none", "apiProvider": "firstParty"}), &[]);
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    let event = wait_for(&rx, |e| matches!(e, Event::Failed(_)));
    assert!(matches!(event, Event::Failed(SessionError::SignInRequired(_))));
    session.stop(Duration::from_millis(200));
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
}

#[test]
fn a_refused_rewind_ends_the_session_with_its_own_error_and_no_crash_report() {
    let dir = scratch("rewind-rejected");
    let refusal =
        "Resume rejected by --resume-drops-turn: resuming at a-1 would discard entries not attributable to turn p-2";
    let mut launch = stand_in(
        &dir,
        json!({"subscriptionType": "pro"}),
        &[
            json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": null, "errors": [refusal], "num_turns": 0, "session_id": "s-2"}),
        ],
    );
    launch.env.push(("THEN_EXIT".into(), "1".into()));
    launch.rewind(
        "s-1",
        &ide_thread::Rewind {
            fork_at: "a-1".into(),
            drops_turn: "p-2".into(),
        },
    );
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    let event = wait_for(&rx, |e| matches!(e, Event::Failed(_)));
    assert_eq!(event, Event::Failed(SessionError::RewindRejected(refusal.into())));
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
    let thread = session.snapshot();
    let errors = thread
        .entries(&Lane::Main)
        .iter()
        .filter(|e| matches!(e.kind, EntryKind::Notice(ide_thread::Notice::Error { .. })))
        .count();
    assert_eq!(errors, 0, "the refusal is not reported as a crash of the process");
    let args = std::fs::read_to_string(dir.join("written.ndjson.args")).unwrap();
    assert!(args.lines().any(|a| a == "--resume-drops-turn=p-2"));
}

#[test]
fn after_a_turn_the_session_asks_how_full_the_context_is_and_keeps_the_breakdown() {
    let dir = scratch("context");
    let launch = stand_in(
        &dir,
        json!({"subscriptionType": "pro"}),
        &[
            json!({"type": "system", "subtype": "init", "session_id": "s-3", "cwd": "/w", "model": "claude-haiku-5-5", "permissionMode": "default", "tools": [], "claude_code_version": "2.1.321"}),
            json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 900, "total_cost_usd": 0.002, "modelUsage": {"claude-haiku-5-5": {"inputTokens": 4, "outputTokens": 9, "costUSD": 0.002, "contextWindow": 200000}}, "session_id": "s-3"}),
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "req-2", "response": {"categories": [{"id": "messages", "name": "Messages", "tokens": 1200, "kind": "used"}, {"id": "free_space", "name": "Free space", "tokens": 198800, "kind": "free"}], "totalTokens": 1200, "rawMaxTokens": 200000, "percentage": 0.6, "model": "claude-haiku-5-5"}}}),
        ],
    );
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    wait_for(&rx, |_| session.with_thread(|t| t.usage().breakdown.is_some()));
    let usage = session.with_thread(|t| t.usage().clone());
    assert_eq!(usage.context_used, Some(1_200));
    assert_eq!(usage.breakdown.map(|b| (b.rows.len(), b.exact)), Some((2, false)));
    assert_eq!(usage.turns.len(), 1);
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
    let asked: Vec<Value> = written(&dir)
        .into_iter()
        .filter(|f| f["request"]["subtype"] == "get_context_usage")
        .collect();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["request"]["detail"], "summary");
}

#[test]
fn a_missing_executable_is_reported_before_anything_starts() {
    let launch = Launch::new("/nonexistent/claude", Vec::new(), "/tmp");
    let result = Session::start(
        &Supervisor::new(),
        &launch,
        Thread::new(ThreadId::new(), "claude", "/tmp"),
        |_| {},
    );
    assert!(matches!(result, Err(SessionError::NotFound(_))));
}

#[test]
#[ignore = "runs the installed claude CLI: needs a sign-in and the network, and costs a few tokens"]
fn the_real_cli_answers_a_prompt() {
    let executable =
        std::env::var_os("CLAUDE_BIN").map_or_else(|| ide_core::paths::home().join(".local/bin/claude"), PathBuf::from);
    let dir = scratch("real");
    let env: Vec<(String, String)> = std::env::vars().collect();
    let mut launch = Launch::new(executable, env, &dir);
    launch.model = Some("haiku".into());
    launch.effort = Some("low".into());
    launch.persist = false;
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    session
        .send(UserMessage {
            text: "Reply with the single word OK.".into(),
            ..UserMessage::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        let _ = rx.recv_timeout(Duration::from_millis(500));
        let done = session.with_thread(|t| {
            t.status() == Status::Idle
                && t.entries(&Lane::Main)
                    .iter()
                    .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown.contains("OK")))
        });
        if done {
            break;
        }
    }
    let thread = session.snapshot();
    assert!(thread.session().is_some());
    assert!(thread.usage().cost_usd.is_some());
    assert!(
        thread
            .entries(&Lane::Main)
            .iter()
            .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown.contains("OK")))
    );
    let caps = session.capabilities();
    assert!(caps.version.is_some());
    assert!(!caps.models.is_empty(), "initialize was answered");
    assert!(session.context_usage(false).wait().is_ok());
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
}

fn remote_answer() -> Value {
    json!({"commands": [], "models": [], "account": {}, "output_style": "default", "remote_control_available": true, "remote_control_auto_enable": true})
}

fn remote_requests(dir: &Path) -> Vec<Value> {
    written(dir)
        .into_iter()
        .filter(|f| f.pointer("/request/subtype") == Some(&json!("remote_control")))
        .map(|f| f["request"].clone())
        .collect()
}

#[test]
fn remote_control_connects_as_the_settings_ask_recovers_and_shows_prompts_from_the_phone() {
    let dir = scratch("remote");
    let launch = stand_in_answering(
        &dir,
        remote_answer(),
        &[
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "req-2", "response": {"session_url": "https://claude.ai/code/s-1", "bridge_epoch": 3}}}),
            json!({"type": "system", "subtype": "bridge_state", "state": "failed", "bridge_epoch": 3, "detail": "network"}),
            json!({"type": "system", "subtype": "bridge_state", "state": "connected", "bridge_epoch": 3, "recovered": true}),
            json!({"type": "user", "uuid": "t-1", "isReplay": true, "origin": {"kind": "task-notification"}, "message": {"role": "user", "content": "a task ended"}}),
            json!({"type": "user", "uuid": "p-1", "isReplay": true, "message": {"role": "user", "content": "from the phone"}}),
        ],
    );
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    let texts = |t: &Thread| -> Vec<String> {
        t.entries(&Lane::Main)
            .iter()
            .filter_map(|e| match &e.kind {
                EntryKind::UserMessage(m) => Some(m.text.clone()),
                _ => None,
            })
            .collect()
    };
    wait_for(&rx, |_| {
        session.remote_control() == RemoteControl::Connected("https://claude.ai/code/s-1".into())
            && !session.with_thread(texts).is_empty()
    });
    assert_eq!(session.with_thread(texts), ["from the phone"]);
    let sent = session.send(UserMessage {
        text: "from the app".into(),
        ..UserMessage::default()
    });
    assert!(sent.is_ok());
    session.set_remote_control(false);
    assert_eq!(session.remote_control(), RemoteControl::Off);
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
    assert_eq!(session.remote_control(), RemoteControl::Unavailable);
    assert_eq!(
        remote_requests(&dir),
        [
            json!({"subtype": "remote_control", "enabled": true}),
            json!({"subtype": "remote_control", "enabled": false})
        ]
    );
    let args = std::fs::read_to_string(dir.join("written.ndjson.args")).unwrap();
    assert!(args.lines().any(|a| a == "--replay-user-messages"));
}

#[test]
fn a_thread_that_turned_remote_control_off_stays_off_when_it_starts_again() {
    let dir = scratch("remote-off");
    let mut launch = stand_in_answering(&dir, remote_answer(), &[]);
    launch.remote_control = Some(false);
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    wait_for(&rx, |e| matches!(e, Event::RemoteControl));
    assert_eq!(session.remote_control(), RemoteControl::Off);
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
    assert!(remote_requests(&dir).is_empty());
}

#[test]
fn remote_control_fails_when_its_connection_breaks_off_but_not_for_an_older_one() {
    let dir = scratch("remote-failed");
    let launch = stand_in_answering(
        &dir,
        remote_answer(),
        &[
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "req-2", "response": {"session_url": "https://claude.ai/code/s-2", "bridge_epoch": 2}}}),
            json!({"type": "system", "subtype": "bridge_state", "state": "failed", "bridge_epoch": 1, "detail": "old"}),
            json!({"type": "system", "subtype": "bridge_state", "state": "failed", "bridge_epoch": 2, "detail": "network"}),
        ],
    );
    let supervisor = Supervisor::new();
    let (session, rx) = start(&launch, &supervisor);
    wait_for(&rx, |_| matches!(session.remote_control(), RemoteControl::Failed(_)));
    assert_eq!(session.remote_control(), RemoteControl::Failed("network".into()));
    session.close_input();
    wait_for(&rx, |e| matches!(e, Event::Ended { .. }));
}
