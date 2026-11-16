//! Sessions against the stand-in server of this crate: the handshake in order, a whole turn with every kind of
//! notification, approvals, questions, cancelling, a crash, a failed turn, resuming, signing in, the version gate,
//! the settings the next turn carries, the management calls, and the shared server Remote Control reaches.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_codex::test_server::{COMMAND_OUTPUT, CRASH, PAIRING_CODE, PHONE_ASK, SERVER_NAME, THREAD};
use ide_codex::{Choice, Event, Launch, LoginStarted, RemoteControl, Session, SessionError, TESTED};
use ide_process::Supervisor;
use ide_thread::{
    Answer, Decision, EntryKind, Lane, LimitStatus, Notice, RequestState, Status, Thread, ThreadId, TodoStatus,
    ToolOutput, ToolStatus, ToolView, UserMessage,
};
use serde_json::Value;

#[derive(Default)]
struct Events(Mutex<Vec<Event>>);

impl Events {
    fn has(&self, wanted: impl Fn(&Event) -> bool) -> bool {
        self.0.lock().unwrap().iter().any(wanted)
    }
}

struct Run {
    session: Session,
    events: Arc<Events>,
    log: PathBuf,
    _supervisor: Supervisor,
}

impl Run {
    /// What the client sent, line by line.
    fn sent(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    fn sent_methods(&self) -> Vec<String> {
        self.sent()
            .iter()
            .filter_map(|m| m["method"].as_str().map(str::to_string))
            .collect()
    }

    fn last(&self, method: &str) -> Value {
        self.sent()
            .into_iter()
            .rev()
            .find(|m| m["method"] == method)
            .unwrap_or(Value::Null)
    }
}

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-codex-session-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn launch(scenario: &str, name: &str) -> (Launch, PathBuf) {
    let dir = folder(name);
    let log = dir.join("sent.ndjson");
    let _ = std::fs::remove_file(&log);
    let env = vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("CODEX_TEST_SCENARIO".into(), scenario.into()),
        ("CODEX_TEST_LOG".into(), log.to_string_lossy().into_owned()),
    ];
    (Launch::new(env!("CARGO_BIN_EXE_codex-test-server"), env, dir), log)
}

fn start(launch: Launch, log: PathBuf) -> Run {
    let supervisor = Supervisor::new();
    let events = Arc::new(Events::default());
    let heard = events.clone();
    let thread = Thread::new(ThreadId::from("t"), ide_codex::AGENT, launch.cwd.clone());
    let session = Session::start(&supervisor, launch, thread, move |e| heard.0.lock().unwrap().push(e)).unwrap();
    Run {
        session,
        events,
        log,
        _supervisor: supervisor,
    }
}

fn wait(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !check() {
        assert!(Instant::now() < deadline, "waited in vain: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn ready(scenario: &str, name: &str) -> Run {
    let (launch, log) = launch(scenario, name);
    let run = start(launch, log);
    wait("an open thread", || run.session.is_ready());
    run
}

fn message(text: &str) -> UserMessage {
    UserMessage {
        text: text.into(),
        ..UserMessage::default()
    }
}

fn kinds(session: &Session) -> Vec<EntryKind> {
    session.with_thread(|t| t.entries(&Lane::Main).iter().map(|e| e.kind.clone()).collect())
}

fn pending_request(session: &Session) -> Option<String> {
    session.with_thread(|t| {
        t.entries(&Lane::Main).iter().find_map(|e| match &e.kind {
            EntryKind::PermissionRequest(p) if p.state == RequestState::Pending => Some(p.request_id.clone()),
            EntryKind::Question(q) if q.state == RequestState::Pending => q.request_id.clone(),
            _ => None,
        })
    })
}

fn await_request(session: &Session) -> String {
    wait("a request", || pending_request(session).is_some());
    pending_request(session).unwrap()
}

fn idle(session: &Session) -> bool {
    session.with_thread(|t| t.status() == Status::Idle)
}

#[test]
fn the_handshake_runs_in_order_and_opens_the_thread() {
    let run = ready("basic", "handshake");
    wait("the models", || !run.session.capabilities().models.is_empty());
    let methods = run.sent_methods();
    assert_eq!(
        methods[..5],
        [
            "initialize",
            "initialized",
            "account/read",
            "thread/start",
            "model/list"
        ],
        "{methods:?}"
    );
    let sent = run.sent();
    assert!(sent.iter().all(|m| m.get("jsonrpc").is_none()), "{sent:?}");
    assert_eq!(sent[0]["params"]["clientInfo"]["name"], ide_product::NAME);
    assert_eq!(
        sent[3]["params"]["cwd"],
        run.session.capabilities().cwd.unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(run.session.session_id().as_deref(), Some(THREAD));
    let caps = run.session.capabilities();
    assert_eq!(caps.version, Some(TESTED));
    assert_eq!(caps.mode(), Some("auto"));
    assert!(caps.signed_in());
    assert_eq!(caps.efforts().len(), 3);
    run.session.with_thread(|t| {
        assert_eq!(t.session(), Some(THREAD));
        assert_eq!(t.model(), Some("gpt-5.5"));
        assert_eq!(t.effort(), Some("medium"));
        assert_eq!(t.mode(), Some("auto"));
        assert!(t.usage().subscription);
    });
    assert!(!run.events.has(|e| matches!(e, Event::Failed(_))));
}

#[test]
fn a_whole_turn_fills_the_thread_and_answers_go_out_as_the_protocol_names_them() {
    let run = ready("basic", "turn");
    run.session.send(message("list the files")).unwrap();
    let command = await_request(&run.session);
    let kinds_now = kinds(&run.session);
    assert!(matches!(&kinds_now[0], EntryKind::UserMessage(m) if m.text == "list the files"));
    assert!(matches!(&kinds_now[1], EntryKind::Thought(t) if t.text == "Reading the files." && !t.streaming));
    assert!(matches!(&kinds_now[2], EntryKind::AgentMessage(m) if m.markdown == "Let me look." && !m.streaming));
    let EntryKind::PermissionRequest(ask) = kinds_now.last().unwrap() else {
        panic!("{kinds_now:?}")
    };
    assert_eq!(
        ask.options.len(),
        4,
        "accept, for the session, the proposed rule, decline"
    );
    assert_eq!(ask.reason.as_deref(), Some("list the folder"));
    assert_eq!(run.session.with_thread(Thread::status), Status::RequiresAction);
    run.session.answer(&command, Choice::Option(1)).unwrap();
    let file = {
        wait("the file change request", || {
            pending_request(&run.session).is_some_and(|r| r != command)
        });
        pending_request(&run.session).unwrap()
    };
    run.session.answer(&file, Choice::Option(0)).unwrap();
    wait("the end of the turn", || {
        idle(&run.session) && run.session.with_thread(|t| t.title() == "Listing files")
    });
    let answers: Vec<Value> = run.sent().into_iter().filter(|m| m.get("result").is_some()).collect();
    assert_eq!(
        answers[0]["result"],
        serde_json::json!({"decision": "acceptForSession"})
    );
    assert_eq!(answers[1]["result"], serde_json::json!({"decision": "accept"}));
    let entries = kinds(&run.session);
    let tool = entries.iter().find_map(|k| match k {
        EntryKind::ToolCall(c) if c.name == "Bash" => Some(c.clone()),
        _ => None,
    });
    let tool = tool.unwrap();
    assert_eq!(tool.status, ToolStatus::Completed);
    assert!(matches!(&tool.view, ToolView::Command { command, .. } if command == "ls -la"));
    assert!(matches!(&tool.output, Some(ToolOutput::Terminal { stdout, .. }) if stdout == COMMAND_OUTPUT));
    let edit = entries.iter().find_map(|k| match k {
        EntryKind::ToolCall(c) if c.name == "Edit" => Some(c.clone()),
        _ => None,
    });
    assert!(
        matches!(edit.unwrap().output, Some(ToolOutput::Diff { hunks, .. }) if hunks[0].lines == [" one", "-two", "+2"])
    );
    assert!(entries.iter().any(|k| matches!(k, EntryKind::PermissionRequest(p)
        if p.state == RequestState::Decided(Decision::Allowed { always: true, edited: false }))));
    assert!(matches!(entries.last(), Some(EntryKind::AgentMessage(m)) if m.markdown == "Done."));
    run.session.with_thread(|t| {
        assert!(t.todos().iter().all(|todo| todo.status == TodoStatus::Completed));
        let usage = t.usage();
        assert_eq!(
            (usage.input_tokens, usage.cache_read_tokens, usage.output_tokens),
            (4000, 8000, 600)
        );
        assert_eq!((usage.context_used, usage.context_size), (Some(6300), Some(272000)));
        assert_eq!(usage.turns.len(), 1);
        assert_eq!(usage.turns[0].duration, Some(Duration::from_millis(1200)));
        let five = t.rate_limits().iter().find(|l| l.window == "five_hour").unwrap();
        assert_eq!(five.status, LimitStatus::Warning);
    });
    assert!(run.session.turn_diff().unwrap().contains("+2"));
}

#[test]
fn a_declined_command_fails_its_card_and_dismissing_stops_the_turn() {
    let run = ready("basic", "decline");
    run.session.send(message("list the files")).unwrap();
    let command = await_request(&run.session);
    run.session.answer(&command, Choice::Option(3)).unwrap();
    wait("the file change request", || {
        pending_request(&run.session).is_some_and(|r| r != command)
    });
    let tool_failed = kinds(&run.session)
        .iter()
        .any(|k| matches!(k, EntryKind::ToolCall(c) if c.name == "Bash" && c.status == ToolStatus::Failed));
    assert!(tool_failed);
    let file = pending_request(&run.session).unwrap();
    run.session.answer(&file, Choice::Dismiss).unwrap();
    wait("the interrupted turn", || idle(&run.session));

    let answers: Vec<Value> = run.sent().into_iter().filter(|m| m.get("result").is_some()).collect();
    assert_eq!(answers[0]["result"]["decision"], "decline");
    assert_eq!(answers[1]["result"]["decision"], "cancel");
    assert!(
        kinds(&run.session)
            .iter()
            .any(|k| matches!(k, EntryKind::Notice(Notice::Interrupted)))
    );
    assert_eq!(
        run.session.answer("p99", Choice::Dismiss),
        Err(SessionError::UnknownRequest("p99".into()))
    );
}

#[test]
fn cancelling_interrupts_the_running_turn() {
    let run = ready("basic", "cancel");
    run.session.send(message("slow please")).unwrap();
    wait("the streaming answer", || {
        kinds(&run.session)
            .iter()
            .any(|k| matches!(k, EntryKind::AgentMessage(m) if m.markdown == "Working"))
    });
    wait("the turn's id", || {
        run.sent_methods().contains(&"turn/start".to_string())
    });
    run.session.cancel();
    wait("the stopped turn", || idle(&run.session));
    let interrupt = run.last("turn/interrupt");
    assert_eq!(interrupt["params"]["threadId"], THREAD);
    assert_eq!(interrupt["params"]["turnId"], "turn-1");
    assert!(
        kinds(&run.session)
            .iter()
            .any(|k| matches!(k, EntryKind::Notice(Notice::Interrupted)))
    );
}

#[test]
fn a_message_during_a_turn_steers_it() {
    let run = ready("basic", "steer");
    run.session.send(message("slow please")).unwrap();
    wait("the running turn", || {
        kinds(&run.session)
            .iter()
            .any(|k| matches!(k, EntryKind::AgentMessage(_)))
    });
    run.session.send(message("and the hidden ones")).unwrap();
    wait("the steer", || !run.last("turn/steer").is_null());
    let steer = run.last("turn/steer");
    assert_eq!(steer["params"]["expectedTurnId"], "turn-1");
    std::thread::sleep(Duration::from_millis(100));
    let users = kinds(&run.session)
        .iter()
        .filter(|k| matches!(k, EntryKind::UserMessage(_)))
        .count();
    assert_eq!(users, 2, "the echo of the steered message is not shown twice");
    run.session.cancel();
}

#[test]
fn questions_are_answered_by_their_ids() {
    let run = ready("basic", "ask");
    run.session.send(message("ask me")).unwrap();
    let request = await_request(&run.session);
    assert_eq!(
        run.session.answer(&request, Choice::Option(0)),
        Err(SessionError::UnknownRequest(request.clone()))
    );
    let answer = Answer {
        question: "Which colour?".into(),
        answer: "Red".into(),
    };
    run.session.answer_questions(&request, vec![answer]).unwrap();
    wait("the end of the turn", || idle(&run.session));
    let answers: Vec<Value> = run.sent().into_iter().filter(|m| m.get("result").is_some()).collect();
    assert_eq!(
        answers[0]["result"],
        serde_json::json!({"answers": {"color": {"answers": ["Red"]}}})
    );
    assert!(
        kinds(&run.session)
            .iter()
            .any(|k| matches!(k, EntryKind::AgentMessage(m) if m.markdown == "You chose Red."))
    );
}

#[test]
fn a_failed_turn_shows_one_error_and_a_crash_the_end_of_stderr() {
    let run = ready("basic", "fail");
    run.session.send(message("fail now")).unwrap();
    wait("the failed turn", || idle(&run.session));
    let errors: Vec<Notice> = kinds(&run.session)
        .into_iter()
        .filter_map(|k| match k {
            EntryKind::Notice(n @ Notice::Error { .. }) => Some(n),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    run.session.send(message("crash now")).unwrap();
    wait("the end", || {
        run.session.is_ended() && run.events.has(|e| matches!(e, Event::Ended { .. }))
    });
    wait("the process notice", || {
        kinds(&run.session).iter().any(|k| {
            matches!(k, EntryKind::Notice(Notice::Error { kind: ide_thread::ErrorKind::Process, message })
                if message.contains(CRASH))
        })
    });
    assert!(idle(&run.session));
    assert_eq!(run.session.send(message("again")), Err(SessionError::Closed));
}

#[test]
fn resuming_replaces_the_thread_with_its_history() {
    let (mut launch, log) = launch("basic", "resume");
    launch.resume = Some(THREAD.into());
    let run = start(launch, log);
    wait("an open thread", || run.session.is_ready());
    assert_eq!(run.last("thread/resume")["params"]["threadId"], THREAD);
    let entries = kinds(&run.session);
    assert!(matches!(&entries[0], EntryKind::UserMessage(m) if m.text == "Earlier question"));
    assert!(matches!(&entries[1], EntryKind::AgentMessage(m) if m.markdown == "Earlier answer" && !m.streaming));
    assert_eq!(run.session.with_thread(|t| t.title().to_string()), "Earlier");
}

#[test]
fn without_an_account_the_session_asks_for_a_sign_in_and_still_opens() {
    let run = ready("signin", "signin");
    assert!(run.events.has(|e| *e == Event::Failed(SessionError::SignIn)));
    assert!(!run.session.capabilities().signed_in());
    let started = run.session.login(&ide_codex::Login::ChatGpt).wait().unwrap();
    assert!(matches!(started, LoginStarted::Browser { login_id, .. } if login_id == "l1"));
    wait("the completed sign-in", || {
        run.events
            .has(|e| matches!(e, Event::Login { success: true, login_id: Some(id), .. } if id == "l1"))
    });
}

#[test]
fn an_older_codex_is_refused_so_the_app_can_fall_back_to_acp() {
    let (launch, log) = launch("old", "old");
    let run = start(launch, log);
    wait("the refusal", || {
        run.events
            .has(|e| matches!(e, Event::Failed(SessionError::Unsupported { .. })))
    });
    assert!(run.events.has(
        |e| matches!(e, Event::Failed(SessionError::Unsupported { found: Some(v), .. })
        if v.to_string() == "0.150.0")
    ));
    wait("the end", || run.session.is_ended());
    assert!(!run.sent_methods().contains(&"thread/start".to_string()));
    let probe = ide_codex::version::probe(
        &Supervisor::new(),
        std::path::Path::new(env!("CARGO_BIN_EXE_codex-test-server")),
        &[("CODEX_TEST_SCENARIO".into(), "basic".into())],
    );
    assert_eq!(probe, Ok(TESTED));
}

#[test]
fn changed_settings_go_out_with_the_next_turn_only() {
    let run = ready("basic", "settings");
    run.session.set_mode("read-only").unwrap();
    run.session.set_option("model", "gpt-5.5-mini").unwrap();
    run.session.set_effort(Some("low")).unwrap();
    assert_eq!(
        run.session.set_mode("yolo"),
        Err(SessionError::UnknownOption("yolo".into()))
    );
    run.session.with_thread(|t| {
        assert_eq!(
            (t.mode(), t.model(), t.effort()),
            (Some("read-only"), Some("gpt-5.5-mini"), Some("low"))
        );
    });
    run.session.send(message("ask me")).unwrap();
    let request = await_request(&run.session);
    let start = run.last("turn/start");
    let params = &start["params"];
    assert_eq!(params["model"], "gpt-5.5-mini");
    assert_eq!(params["effort"], "low");
    assert_eq!(params["approvalPolicy"], "on-request");
    assert_eq!(params["sandboxPolicy"], serde_json::json!({"type": "readOnly"}));
    assert!(
        params["clientUserMessageId"]
            .as_str()
            .is_some_and(|k| k.starts_with("user-"))
    );
    run.session.answer(&request, Choice::Dismiss).unwrap();
    wait("the end of the turn", || idle(&run.session));
    run.session.send(message("ask again")).unwrap();
    let _ = await_request(&run.session);
    let params = run.last("turn/start")["params"].clone();
    assert!(
        params.get("model").is_none() && params.get("sandboxPolicy").is_none(),
        "{params}"
    );
}

#[test]
fn management_calls_answer_in_their_types() {
    let run = ready("basic", "manage");
    let models = run.session.models(false).wait().unwrap();
    assert_eq!(models.len(), 2);
    let page = run.session.threads(&ide_codex::ThreadQuery::default()).wait().unwrap();
    assert_eq!(page.threads[0].id, THREAD);
    let fork = run.session.fork(Some("turn-1")).unwrap().wait().unwrap();
    assert_eq!(fork.forked_from_id.as_deref(), Some(THREAD));
    assert_eq!(run.last("thread/fork")["params"]["lastTurnId"], "turn-1");
    let written = run
        .session
        .write_config("model", serde_json::json!("gpt-5.5"), ide_codex::MergeStrategy::Replace)
        .wait()
        .unwrap();
    assert_eq!(written.status, "ok");
    assert_eq!(run.last("config/value/write")["params"]["mergeStrategy"], "replace");
    let servers = run.session.mcp_servers().wait().unwrap();
    assert_eq!(servers[0].name, "docs");
    run.session.reload_mcp().wait().unwrap();
    assert!(run.last("config/mcpServer/reload").get("params").is_none());
    let skills = run.session.skills(false).wait().unwrap();
    assert_eq!(skills[0].skills[0].name, "review");
    assert!(!run.session.set_skill(&skills[0].skills[0].path, false).wait().unwrap());
    let account = run.session.account().wait().unwrap();
    assert!(account.account.is_some_and(|a| a.is_subscription()));
    let limits = run.session.rate_limits().wait().unwrap();
    assert_eq!(limits[0].window, "five_hour");
    wait("the limits on the thread", || {
        run.session.with_thread(|t| !t.rate_limits().is_empty())
    });
    let feedback = ide_codex::Feedback {
        classification: "bug".into(),
        reason: None,
        include_logs: false,
    };
    assert_eq!(run.session.upload_feedback(&feedback).wait().unwrap().thread_id, THREAD);
    assert!(run.session.read_config(false).wait().unwrap()["config"]["model"].is_string());
    assert!(run.session.plugins().wait().is_ok());
    let unknown = run.session.request("nonexistent/method", Value::Null).wait();
    assert!(matches!(unknown, Err(SessionError::Server(e)) if e.code == -32600));
    run.session.logout().wait().unwrap();
    assert_eq!(run.session.capabilities().account, None);
}

#[test]
fn stopping_ends_the_session_quietly() {
    let run = ready("basic", "stop");
    run.session.stop(Duration::from_millis(200));
    wait("the end", || run.events.has(|e| matches!(e, Event::Ended { .. })));
    assert!(run.session.is_ended());
    assert!(!kinds(&run.session).iter().any(|k| matches!(k, EntryKind::Notice(_))));
    assert_eq!(run.session.models(false).wait(), Err(SessionError::Closed));
}

#[test]
fn a_stop_while_a_refused_turn_starts_leaves_the_next_turn_alone() {
    let run = ready("basic", "refused-stop");
    run.session.send(message("refuse this")).unwrap();
    run.session.cancel();
    wait("the refusal", || {
        run.session.snapshot().status() == Status::Idle && run.sent_methods().iter().any(|m| m == "turn/start")
    });
    run.session.send(message("list the files")).unwrap();
    wait("the next turn asks for its command", || {
        pending_request(&run.session).is_some()
    });
    let methods = run.sent_methods();
    assert!(!methods.iter().any(|m| m == "turn/interrupt"), "{methods:?}");
}

#[test]
fn a_version_read_before_the_start_is_not_read_again() {
    let (mut launch, log) = launch("basic", "known-version");
    launch.version = ide_codex::Version::parse("codex-cli 0.100.0");
    let run = start(launch, log);
    wait("the refusal", || {
        run.events
            .has(|e| matches!(e, Event::Failed(SessionError::Unsupported { .. })))
    });
}

/// A launch of the shared server whose Remote Control the user turned on, with Codex's files in the test's folder.
fn shared(name: &str) -> (Launch, PathBuf) {
    let (mut launch, log) = launch("basic", name);
    let home = launch.cwd.join("codex-home");
    let _ = std::fs::remove_dir_all(&home);
    launch
        .env
        .push(("CODEX_HOME".into(), home.to_string_lossy().into_owned()));
    (launch, log)
}

#[test]
fn remote_control_starts_through_the_cli_and_pairs_a_phone() {
    let (launch, _) = shared("remote-cli");
    let supervisor = Supervisor::new();
    assert!(!ide_codex::remote::wanted(&launch.env));
    let started = ide_codex::remote::start(&supervisor, &launch.program, &launch.env).unwrap();
    assert_eq!(started, RemoteControl::Connected(SERVER_NAME.into()));
    assert!(
        ide_codex::remote::wanted(&launch.env),
        "the choice is kept for the daemon's next start"
    );
    ide_codex::remote::ensure_daemon(&supervisor, &launch.program, &launch.env).unwrap();
    let pairing = ide_codex::remote::pair(&supervisor, &launch.program, &launch.env).unwrap();
    assert_eq!(pairing.code, PAIRING_CODE);
    let missing = ide_codex::remote::start(&supervisor, &launch.cwd.join("codex"), &launch.env);
    assert!(matches!(missing, Err(SessionError::NotFound(_))));
}

#[test]
fn a_shared_session_shows_remote_control_leaves_other_threads_alone_and_turns_it_off() {
    let (mut launch, log) = shared("remote-session");
    let supervisor = Supervisor::new();
    ide_codex::remote::start(&supervisor, &launch.program, &launch.env).unwrap();
    launch.shared = true;
    assert_eq!(launch.args(), ["app-server", "proxy"]);
    let env = launch.env.clone();
    let run = start(launch, log);
    wait("an open thread", || run.session.is_ready());
    assert_eq!(run.sent()[0]["params"]["capabilities"]["experimentalApi"], true);
    wait("Remote Control", || {
        run.session.remote_control() == RemoteControl::Connected(SERVER_NAME.into())
    });
    assert!(run.events.has(|e| matches!(e, Event::RemoteControl)));
    run.session.send(message("foreign: list the files")).unwrap();
    let command = await_request(&run.session);
    let asks = kinds(&run.session)
        .into_iter()
        .filter(|k| matches!(k, EntryKind::PermissionRequest(_)))
        .count();
    assert_eq!(asks, 1, "the phone's request stays out of the thread");
    run.session.answer(&command, Choice::Option(0)).unwrap();
    wait("the file change request", || {
        pending_request(&run.session).is_some_and(|r| r != command)
    });
    assert!(
        run.sent().iter().all(|m| m["id"] != PHONE_ASK),
        "the phone answers its own request"
    );
    let off = run.session.disable_remote_control().unwrap().wait().unwrap();
    assert_eq!(off, RemoteControl::Off);
    assert_eq!(run.session.remote_control(), RemoteControl::Off);
    assert!(!ide_codex::remote::wanted(&env));
}

#[test]
fn a_session_of_its_own_has_no_remote_control() {
    let run = ready("basic", "remote-own");
    assert_eq!(run.sent()[0]["params"]["capabilities"]["experimentalApi"], false);
    assert_eq!(run.session.remote_control(), RemoteControl::Off);
    assert!(run.session.disable_remote_control().is_err());
    assert!(!run.events.has(|e| matches!(e, Event::RemoteControl)));
}
