//! The agent's turn against a stub provider in a scratch project: tools that read, edit and run, the approvals the
//! policy asks for, the sandbox, retries, cancelling and resuming a stored thread.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_agent::stub::{Answer, Stub};
use ide_agent::{
    ApiKey, Approval, Choice, Config, Endpoint, Event, Http, Policy, PriceTable, Profile, Provider, ProviderKind, Rule,
    Session, Workspace,
};
use ide_thread::{
    Decision, Entry, EntryKind, Lane, Notice, PermissionOption, RequestState, Status, Thread, ThreadId, TodoStatus,
    ToolOutput, ToolStatus, UserMessage,
};
use serde_json::{Value, json};

/// A scratch project with a file to work on, removed when dropped.
struct Project(PathBuf);

impl Project {
    fn new(name: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("ide-agent-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("project/src")).unwrap();
        std::fs::write(
            dir.join("project/src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        Project(dir)
    }

    fn root(&self) -> PathBuf {
        self.0.join("project").canonicalize().unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn event(name: &str, value: &Value) -> String {
    format!("event: {name}\ndata: {value}\n\n")
}

/// An Anthropic stream that calls `tools`, given as id, name and input.
fn calls(tools: &[(&str, &str, Value)]) -> Answer {
    let mut text = event(
        "message_start",
        &json!({"type": "message_start", "message": {"model": "claude-opus-5-5", "usage": {"input_tokens": 100, "output_tokens": 1}}}),
    );
    for (index, (id, name, input)) in tools.iter().enumerate() {
        text.push_str(&event(
            "content_block_start",
            &json!({"type": "content_block_start", "index": index, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}),
        ));
        text.push_str(&event(
            "content_block_delta",
            &json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": input.to_string()}}),
        ));
        text.push_str(&event(
            "content_block_stop",
            &json!({"type": "content_block_stop", "index": index}),
        ));
    }
    text.push_str(&event(
        "message_delta",
        &json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 20}}),
    ));
    text.push_str(&event("message_stop", &json!({"type": "message_stop"})));
    Answer::events(&text)
}

/// An Anthropic stream that answers `text` and ends the turn.
fn says(text: &str) -> Answer {
    let mut stream = event(
        "message_start",
        &json!({"type": "message_start", "message": {"model": "claude-opus-5-5", "usage": {"input_tokens": 200, "output_tokens": 1}}}),
    );
    stream.push_str(&event(
        "content_block_start",
        &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
    ));
    stream.push_str(&event(
        "content_block_delta",
        &json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}}),
    ));
    stream.push_str(&event(
        "message_delta",
        &json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 10}}),
    ));
    stream.push_str(&event("message_stop", &json!({"type": "message_stop"})));
    Answer::events(&stream)
}

struct Run {
    session: Session,
    events: Arc<Mutex<Vec<Event>>>,
}

fn config(stub: &Stub, project: &Project, approval: Approval, sandbox: bool, store: Option<PathBuf>) -> Config {
    let endpoint = Endpoint::new("stub", ProviderKind::AnthropicCompatible, "Stub", &stub.url());
    let environment: Vec<(String, String)> = std::env::vars().collect();
    let workspace = Workspace::new(
        &project.root(),
        environment,
        ide_process::Supervisor::new(),
        Http::new(),
    )
    .unwrap();
    Config {
        thread: ThreadId::from("t1"),
        provider: Provider::new(endpoint, ApiKey::new("k"), Http::new()),
        model: "claude-opus-5-5".into(),
        effort: Some("high".into()),
        workspace,
        policy: Policy {
            profile: Profile::Write,
            approval,
            sandbox,
            rules: vec![],
        },
        prices: PriceTable::default(),
        listed_price: Some(ide_agent::Price::new(4.0, 20.0)),
        context_size: Some(1_000_000),
        max_tokens: None,
        store,
        wording: Arc::new(ide_agent::Plain),
    }
}

fn start(config: Config) -> Run {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let session = Session::start(config, move |event| sink.lock().unwrap().push(event));
    Run { session, events }
}

fn send(run: &Run, text: &str) {
    run.session
        .send(UserMessage {
            text: text.into(),
            ..UserMessage::default()
        })
        .unwrap();
}

fn wait_for(run: &Run, what: &str, ready: impl Fn(&Thread) -> bool) -> Thread {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let thread = run.session.snapshot();
        if ready(&thread) {
            return thread;
        }
        assert!(
            Instant::now() < deadline,
            "waited in vain for {what}: {:#?}",
            thread.entries(&Lane::Main)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn idle(run: &Run) -> Thread {
    std::thread::sleep(Duration::from_millis(30));
    wait_for(run, "the turn's end", |t| {
        t.status() == Status::Idle && !run.session.is_running()
    })
}

fn pending_request(run: &Run) -> (String, Vec<PermissionOption>, bool) {
    let thread = wait_for(run, "a permission request", |t| !t.pending_requests().is_empty());
    let entry = thread.pending_requests()[0].clone();
    let EntryKind::PermissionRequest(request) = &entry.kind else {
        panic!()
    };
    (
        request.request_id.clone(),
        request.options.clone(),
        request.default_to_no,
    )
}

fn entries(thread: &Thread) -> Vec<Entry> {
    thread.entries(&Lane::Main).iter().map(|e| (**e).clone()).collect()
}

fn tool(thread: &Thread, id: &str) -> ide_thread::ToolCall {
    match &thread.entry_by_key(id).expect(id).kind {
        EntryKind::ToolCall(call) => call.clone(),
        other => panic!("{other:?}"),
    }
}

fn sent_results(stub: &Stub, request: usize) -> Vec<Value> {
    let body = stub.seen()[request].json();
    let last = body["messages"].as_array().unwrap().last().unwrap().clone();
    last["content"].as_array().unwrap().clone()
}

struct GatedDiagnostics(std::sync::mpsc::Sender<std::sync::mpsc::Sender<()>>);

impl ide_agent::Diagnostics for GatedDiagnostics {
    fn diagnostics(&self, _path: Option<&Path>) -> Vec<ide_agent::Diagnostic> {
        let (release, wait) = std::sync::mpsc::channel();
        self.0.send(release).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        Vec::new()
    }
}

#[test]
fn reads_overlap_four_at_a_time_and_writes_are_ordering_barriers() {
    let project = Project::new("parallel");
    let mut tools = Vec::new();
    for id in ["d0", "d1", "d2", "d3", "d4"] {
        tools.push((id, "diagnostics", json!({})));
    }
    tools.push((
        "write",
        "write_file",
        json!({"path": "barrier.txt", "content": "written"}),
    ));
    tools.push(("after", "diagnostics", json!({})));
    tools.push(("read", "read_file", json!({"path": "barrier.txt"})));
    let stub = Stub::script(vec![calls(&tools), says("done")]).unwrap();
    let (entered, receive) = std::sync::mpsc::channel();
    let mut config = config(&stub, &project, Approval::All, false, None);
    config.workspace = config.workspace.with_diagnostics(Arc::new(GatedDiagnostics(entered)));
    let run = start(config);
    send(&run, "read, write, then read again");
    let first: Vec<_> = (0..4)
        .map(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    assert!(receive.try_recv().is_err(), "a fifth read must wait for a worker slot");
    assert!(!project.root().join("barrier.txt").exists());
    for release in first.into_iter().rev() {
        release.send(()).unwrap();
    }
    let fifth = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        !project.root().join("barrier.txt").exists(),
        "the write waits for every earlier read"
    );
    fifth.send(()).unwrap();
    let after = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        std::fs::read_to_string(project.root().join("barrier.txt")).unwrap(),
        "written"
    );
    after.send(()).unwrap();
    idle(&run);
    let results = sent_results(&stub, 1);
    let ids: Vec<_> = results.iter().map(|r| r["tool_use_id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["d0", "d1", "d2", "d3", "d4", "write", "after", "read"]);
    assert!(results.last().unwrap()["content"].as_str().unwrap().contains("written"));
}

#[test]
fn cancellation_does_not_start_the_next_read_group() {
    let project = Project::new("parallel-cancel");
    let tools: Vec<_> = ["d0", "d1", "d2", "d3", "d4", "d5"]
        .into_iter()
        .map(|id| (id, "diagnostics", json!({})))
        .collect();
    let stub = Stub::script(vec![calls(&tools)]).unwrap();
    let (entered, receive) = std::sync::mpsc::channel();
    let mut config = config(&stub, &project, Approval::All, false, None);
    config.workspace = config.workspace.with_diagnostics(Arc::new(GatedDiagnostics(entered)));
    let run = start(config);
    send(&run, "read");
    let first: Vec<_> = (0..4)
        .map(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    run.session.cancel();
    for release in first {
        release.send(()).unwrap();
    }
    let thread = idle(&run);
    assert!(receive.try_recv().is_err());
    assert_eq!(tool(&thread, "d4").status, ToolStatus::Failed);
    assert_eq!(tool(&thread, "d5").status, ToolStatus::Failed);
}

#[test]
fn a_turn_reads_then_edits_with_the_users_approval_and_stores_a_rule() {
    let project = Project::new("edit");
    let store = project.0.join("threads/t1.json");
    let stub = Stub::script(vec![
        calls(&[("r1", "read_file", json!({"path": "src/main.rs"}))]),
        calls(&[(
            "e1",
            "edit_file",
            json!({"path": "src/main.rs", "old_string": "\"hello\"", "new_string": "\"hello, world\""}),
        )]),
        says("Done: it now greets the world."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, Some(store.clone())));
    send(&run, "Greet the world");
    let (request, options, risky) = pending_request(&run);
    assert!(!risky);
    let always = options
        .iter()
        .position(|o| matches!(o, PermissionOption::AllowAlways { .. }))
        .unwrap();
    run.session.answer(&request, Choice::Option(always)).unwrap();
    let thread = idle(&run);

    let main = std::fs::read_to_string(project.root().join("src/main.rs")).unwrap();
    assert!(main.contains("hello, world"), "{main}");
    let read = tool(&thread, "r1");
    assert_eq!(read.status, ToolStatus::Completed);
    let edit = tool(&thread, "e1");
    assert_eq!(edit.status, ToolStatus::Completed);
    let Some(ToolOutput::Diff { path, hunks }) = &edit.output else {
        panic!("{:?}", edit.output)
    };
    assert_eq!(path, "src/main.rs");
    assert!(hunks[0].lines.iter().any(|l| l == "+    println!(\"hello, world\");"));
    let EntryKind::PermissionRequest(asked) = &thread.entry_by_key(&request).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        asked.state,
        RequestState::Decided(Decision::Allowed {
            always: true,
            edited: false
        })
    );
    assert!(run.events.lock().unwrap().contains(&Event::RuleAdded(Rule::Edits)));
    assert_eq!(run.session.policy().rules, [Rule::Edits]);
    let last = entries(&thread).last().cloned().unwrap();
    assert!(matches!(last.kind, EntryKind::AgentMessage(ref m) if m.markdown == "Done: it now greets the world."));
    assert_eq!(thread.title(), "Greet the world");

    let read_result = &sent_results(&stub, 1)[0];
    assert_eq!(read_result["tool_use_id"], "r1");
    assert!(
        read_result["content"]
            .as_str()
            .unwrap()
            .contains("     2\t    println!(\"hello\");")
    );
    assert_eq!(sent_results(&stub, 2)[0]["is_error"], false);
    let usage = thread.usage();
    assert_eq!((usage.input_tokens, usage.output_tokens), (400, 50));
    let cost = (400.0 * 4.0 + 50.0 * 20.0) / 1e6;
    assert!((usage.cost_usd.unwrap() - cost).abs() < 1e-12);
    let [turn] = &usage.turns[..] else {
        panic!("{:?}", usage.turns)
    };
    assert_eq!(turn.tokens.map(|t| (t.input, t.output)), Some((400, 50)));
    assert!((turn.cost_usd.unwrap() - cost).abs() < 1e-12 && turn.duration.is_some());
    assert!(turn.message.is_some(), "the thread names the message that started it");
    assert_eq!(usage.models.len(), 1);

    let resumed = Session::resume(config(&stub, &project, Approval::Manual, false, Some(store)), |_| {}).unwrap();
    let again = resumed.snapshot();
    assert_eq!(again.title(), "Greet the world");
    let kinds: Vec<&str> = entries(&again)
        .iter()
        .map(|e| match &e.kind {
            EntryKind::UserMessage(_) => "user",
            EntryKind::ToolCall(_) => "tool",
            EntryKind::AgentMessage(_) => "agent",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["user", "tool", "tool", "agent"]);
    assert_eq!(again.usage().output_tokens, 50);
}

#[test]
fn destructive_git_asks_even_when_all_is_allowed_and_a_decline_reaches_the_model() {
    let project = Project::new("git");
    let stub = Stub::script(vec![
        calls(&[("g1", "run_command", json!({"command": "git reset --hard"}))]),
        says("Understood, I left it."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::All, false, None));
    send(&run, "Throw my changes away");
    let (request, options, risky) = pending_request(&run);
    assert!(risky);
    assert_eq!(options, [PermissionOption::AllowOnce, PermissionOption::Deny]);
    let asked = run.session.snapshot();
    let EntryKind::PermissionRequest(asked) = &asked.entry_by_key(&request).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        asked.reason.as_deref(),
        Some("Destructive(ResetHard)"),
        "the app words why it asks"
    );
    run.session.answer(&request, Choice::Option(1)).unwrap();
    let thread = idle(&run);
    let card = tool(&thread, "g1");
    assert_eq!(
        (card.status, card.output),
        (ToolStatus::Failed, Some(ToolOutput::Text("Declined".into())))
    );
    let result = &sent_results(&stub, 1)[0];
    assert_eq!(
        (result["is_error"].clone(), result["content"].clone()),
        (json!(true), json!("The user declined this call."))
    );
}

#[test]
fn the_sandbox_refuses_paths_outside_and_keeps_commands_from_writing_there() {
    let project = Project::new("sandbox");
    let stub = Stub::script(vec![
        calls(&[
            ("o1", "read_file", json!({"path": "../outside.txt"})),
            (
                "c1",
                "run_command",
                json!({"command": "echo in > inside.txt; echo out > ../outside.txt"}),
            ),
        ]),
        says("ok"),
    ])
    .unwrap();
    std::fs::write(project.0.join("outside.txt"), "secret").unwrap();
    let run = start(config(&stub, &project, Approval::All, true, None));
    send(&run, "Try both");
    let thread = idle(&run);
    assert!(
        thread.pending_requests().is_empty(),
        "nothing asks: it is refused or allowed"
    );
    let results = sent_results(&stub, 1);
    assert!(
        results[0]["content"].as_str().unwrap().starts_with("Refused:"),
        "{}",
        results[0]
    );
    assert_eq!(
        std::fs::read_to_string(project.root().join("inside.txt")).unwrap(),
        "in\n"
    );
    assert_eq!(
        std::fs::read_to_string(project.0.join("outside.txt")).unwrap(),
        "secret"
    );
    let command = tool(&thread, "c1");
    assert_eq!(
        command.status,
        ToolStatus::Failed,
        "the shell reports the refused write"
    );
    let Some(ToolOutput::Terminal { stderr, .. }) = command.output else {
        panic!()
    };
    assert!(stderr.contains("not permitted"), "{stderr}");
}

#[test]
fn a_rate_limit_is_waited_out_and_shown() {
    let project = Project::new("retry");
    let stub = Stub::script(vec![
        Answer::json(
            429,
            &json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}}),
        )
        .with_header("retry-after", "0"),
        says("Here I am."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "hi");
    let thread = idle(&run);
    let notices: Vec<Notice> = entries(&thread)
        .iter()
        .filter_map(|e| match &e.kind {
            EntryKind::Notice(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert!(
        matches!(
            &notices[..],
            [Notice::Retrying {
                attempt: 1,
                http_status: Some(429),
                ..
            }]
        ),
        "{notices:?}"
    );
    assert!(matches!(&entries(&thread).last().unwrap().kind, EntryKind::AgentMessage(m) if m.markdown == "Here I am."));
}

#[test]
fn cancelling_while_a_request_waits_withdraws_it_and_ends_the_turn() {
    let project = Project::new("cancel");
    let stub = Stub::script(vec![calls(&[(
        "w1",
        "write_file",
        json!({"path": "new.txt", "content": "x"}),
    )])])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "Write it");
    let (request, _, _) = pending_request(&run);
    run.session.cancel();
    let thread = idle(&run);
    let EntryKind::PermissionRequest(asked) = &thread.entry_by_key(&request).unwrap().kind else {
        panic!()
    };
    assert_eq!(asked.state, RequestState::Withdrawn);
    assert!(!project.root().join("new.txt").exists());
    assert!(
        entries(&thread)
            .iter()
            .any(|e| matches!(e.kind, EntryKind::Notice(Notice::Interrupted)))
    );
    assert_eq!(stub.seen().len(), 1, "no request after the cancel");
}

#[test]
fn searches_the_to_do_list_and_a_fetched_page_run_in_one_reply() {
    let project = Project::new("tools");
    let page = Stub::start(|_| {
        let mut answer = Answer::json(200, &json!(null));
        answer.headers = vec![("content-type".into(), "text/html".into())];
        answer.parts = vec![(
            Duration::ZERO,
            "<html><body><h1>Docs</h1><p>Use &lt;b&gt;</p></body></html>".into(),
        )];
        answer
    })
    .unwrap();
    let stub = Stub::script(vec![
        calls(&[
            ("s1", "grep", json!({"pattern": "println"})),
            ("s2", "glob", json!({"pattern": "**/*.rs"})),
            (
                "d1",
                "todo_write",
                json!({"todos": [
                {"content": "Read the code", "status": "completed"},
                {"content": "Explain it", "status": "in_progress", "active_form": "Explaining it"}]}),
            ),
            ("f1", "web_fetch", json!({"url": format!("{}/docs", page.url())})),
        ]),
        says("It prints hello."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::All, false, None));
    send(&run, "Explain main");
    let thread = idle(&run);
    let results = sent_results(&stub, 1);
    assert_eq!(results.len(), 4, "all results go back in one message");
    assert_eq!(results[0]["content"], "src/main.rs:2: println!(\"hello\");");
    assert_eq!(results[1]["content"], "src/main.rs");
    let fetched = results[3]["content"].as_str().unwrap();
    assert!(
        fetched.contains(ide_thread::EXTERNAL_NOTICE) && fetched.ends_with("Docs\nUse <b>\n</external>"),
        "{fetched}"
    );
    assert_eq!(tool(&thread, "s1").status, ToolStatus::Completed);
    let todos = thread.todos();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[1].status, TodoStatus::InProgress);
}

#[test]
fn a_read_only_thread_is_not_offered_the_writing_tools() {
    let project = Project::new("ask");
    let stub = Stub::script(vec![says("Looks fine.")]).unwrap();
    let mut config = config(&stub, &project, Approval::Manual, false, None);
    config.policy.profile = Profile::Ask;
    let run = start(config);
    send(&run, "Review it");
    idle(&run);
    let body = stub.seen()[0].json();
    let names: Vec<&str> = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        !names.contains(&"edit_file") && !names.contains(&"run_command"),
        "{names:?}"
    );
    assert!(body["system"][0]["text"].as_str().unwrap().contains("may only read"));
    let path: &Path = project.0.as_path();
    assert!(path.exists());
}

fn notices(thread: &Thread) -> Vec<Notice> {
    entries(thread)
        .iter()
        .filter_map(|e| match &e.kind {
            EntryKind::Notice(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

fn first_message(stub: &Stub, request: usize) -> Vec<Value> {
    let body = stub.seen()[request].json();
    body["messages"][0]["content"].as_array().unwrap().clone()
}

#[test]
fn a_nearly_full_window_is_compacted_before_the_next_request_and_the_last_turn_stays() {
    let project = Project::new("compact");
    let store = project.0.join("threads/t1.json");
    let stub = Stub::script(vec![
        calls(&[(
            "e1",
            "edit_file",
            json!({"path": "src/main.rs", "old_string": "hello", "new_string": "hi"}),
        )]),
        says("Changed the greeting."),
        says("They changed the greeting in src/main.rs."),
        says("Second answer."),
    ])
    .unwrap();
    let mut config = config(&stub, &project, Approval::All, false, Some(store.clone()));
    config.context_size = Some(240);
    let run = start(config.clone());
    send(&run, "Shorten the greeting");
    idle(&run);
    send(&run, "Now explain it");
    let thread = idle(&run);

    let asked = stub.seen()[2].json();
    assert!(asked["system"].to_string().contains("You compact"), "{asked}");
    let transcript = asked["messages"][0]["content"].to_string();
    assert!(
        transcript.contains("Shorten the greeting") && transcript.contains("[agent calls edit_file]"),
        "{transcript}"
    );
    assert!(asked["tools"].as_array().is_none_or(Vec::is_empty));
    let sent = first_message(&stub, 3);
    let summary = sent[0]["text"].as_str().unwrap();
    assert!(summary.starts_with("<conversation-summary"), "{summary}");
    assert!(summary.contains("They changed the greeting") && summary.contains("Files changed: src/main.rs"));
    assert_eq!(sent[1]["text"], "Now explain it", "the last turn stays as it was");
    assert_eq!(stub.seen()[3].json()["messages"].as_array().unwrap().len(), 1);
    assert!(
        matches!(
            &notices(&thread)[..],
            [Notice::Compaction {
                manual: false,
                tokens_before: 210,
                tokens_after: Some(200)
            }]
        ),
        "{:?}",
        notices(&thread)
    );
    assert!(
        matches!(&entries(&thread).last().unwrap().kind, EntryKind::AgentMessage(m) if m.markdown == "Second answer.")
    );
    assert_eq!(thread.usage().turns.len(), 2);

    let resumed = Session::resume(config, |_| {}).unwrap();
    let kinds: Vec<&str> = entries(&resumed.snapshot())
        .iter()
        .map(|e| match &e.kind {
            EntryKind::Notice(Notice::Compaction { manual: false, .. }) => "compacted",
            EntryKind::UserMessage(_) => "user",
            EntryKind::AgentMessage(_) => "agent",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["compacted", "user", "agent"]);
}

#[test]
fn a_conversation_the_window_no_longer_holds_is_compacted_and_sent_again() {
    let project = Project::new("too-long");
    let stub = Stub::script(vec![
        says("First."),
        Answer::json(
            400,
            &json!({"type": "error", "error": {"type": "invalid_request_error",
                    "message": "prompt is too long: 250000 tokens > 200000 maximum"}}),
        ),
        says("A summary."),
        says("Second."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "one");
    idle(&run);
    send(&run, "two");
    let thread = idle(&run);
    assert!(
        matches!(&notices(&thread)[..], [Notice::Compaction { .. }]),
        "{:?}",
        notices(&thread)
    );
    assert_eq!(first_message(&stub, 3)[1]["text"], "two");
    assert!(matches!(&entries(&thread).last().unwrap().kind, EntryKind::AgentMessage(m) if m.markdown == "Second."));
}

#[test]
fn compact_summarizes_on_the_users_word_and_the_next_message_joins_the_summary() {
    let project = Project::new("manual");
    let stub = Stub::script(vec![says("First."), says("A summary."), says("Next.")]).unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "/compact");
    let thread = idle(&run);
    assert!(
        matches!(&notices(&thread)[..], [Notice::Info { text, .. }] if text == "NothingToCompact"),
        "{:?}",
        notices(&thread)
    );
    assert!(stub.seen().is_empty());
    send(&run, "one");
    idle(&run);
    send(&run, "/compact keep the API stable");
    let thread = idle(&run);
    assert!(
        stub.seen()[1].json()["messages"][0]["content"]
            .to_string()
            .contains("keep the API stable")
    );
    assert!(matches!(
        notices(&thread).last(),
        Some(Notice::Compaction { manual: true, .. })
    ));
    let users = entries(&thread)
        .iter()
        .filter(|e| matches!(e.kind, EntryKind::UserMessage(_)))
        .count();
    assert_eq!(users, 1, "the command is not a message");
    send(&run, "next");
    idle(&run);
    let sent = first_message(&stub, 2);
    assert!(sent[0]["text"].as_str().unwrap().contains("A summary."));
    assert_eq!(sent[1]["text"], "next");
    assert_eq!(stub.seen()[2].json()["messages"].as_array().unwrap().len(), 1);
}

struct OneWarning(PathBuf);

impl ide_agent::Diagnostics for OneWarning {
    fn diagnostics(&self, _path: Option<&Path>) -> Vec<ide_agent::Diagnostic> {
        vec![ide_agent::Diagnostic {
            path: self.0.join("src/main.rs"),
            line: 1,
            column: 4,
            severity: ide_agent::Severity::Warning,
            message: "unused".into(),
            source: Some("rustc".into()),
        }]
    }
}

#[test]
fn a_resumed_thread_words_its_cards_notes_again() {
    let project = Project::new("notes");
    let store = project.0.join("threads/t1.json");
    let stub = Stub::script(vec![
        calls(&[
            ("f1", "read_file", json!({"path": "src"})),
            ("d1", "diagnostics", json!({})),
        ]),
        says("One warning."),
    ])
    .unwrap();
    let mut config = config(&stub, &project, Approval::Manual, false, Some(store));
    config.workspace = config.workspace.with_diagnostics(Arc::new(OneWarning(project.root())));
    let run = start(config.clone());
    send(&run, "Check it");
    let live = idle(&run);
    let results = sent_results(&stub, 1);
    assert_eq!(results[1]["content"], "src/main.rs:1:4: warning: unused [rustc]");
    let resumed = Session::resume(config, |_| {}).unwrap().snapshot();
    for thread in [&live, &resumed] {
        let folder = thread
            .entries(&Lane::Main)
            .iter()
            .find_map(|e| match &e.kind {
                EntryKind::ToolCall(call) if call.name == "read_file" => Some(call.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            (folder.status, folder.output),
            (ToolStatus::Failed, Some(ToolOutput::Text("Folder(\"src\")".into())))
        );
        let found = thread.entries(&Lane::Main).iter().find_map(|e| match &e.kind {
            EntryKind::ToolCall(call) if call.name == "diagnostics" => call.output.clone(),
            _ => None,
        });
        assert!(
            matches!(&found, Some(ToolOutput::Text(text)) if text.starts_with("Diagnostics([Problem")),
            "{found:?}"
        );
    }
}
