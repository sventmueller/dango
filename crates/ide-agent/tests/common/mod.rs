//! What the tests of the agent's turns share: a scratch project, the stub provider's streams and a running session.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_agent::stub::{Answer, Stub};
use ide_agent::{
    ApiKey, Approval, Config, Endpoint, Event, Http, Policy, PriceTable, Profile, Provider, ProviderKind, Session,
    Workspace,
};
use ide_thread::{Entry, EntryKind, Lane, Notice, Status, Thread, ThreadId, UserMessage};
use serde_json::{Value, json};

/// A scratch project with a file to work on, removed when dropped.
pub struct Project(pub ide_core::testing::Scratch);

impl Project {
    pub fn new(name: &str) -> Project {
        let dir = ide_core::testing::scratch(&format!("agent-session-{name}"));
        std::fs::create_dir_all(dir.join("project/src")).unwrap();
        std::fs::write(
            dir.join("project/src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        Project(dir)
    }

    pub fn root(&self) -> PathBuf {
        self.0.join("project").canonicalize().unwrap()
    }
}

pub fn event(name: &str, value: &Value) -> String {
    format!("event: {name}\ndata: {value}\n\n")
}

/// An Anthropic stream that answers `text` and ends the turn.
pub fn says(text: &str) -> Answer {
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

pub struct Run {
    pub session: Session,
    pub events: Arc<Mutex<Vec<Event>>>,
}

pub fn config(stub: &Stub, project: &Project, approval: Approval, sandbox: bool, store: Option<PathBuf>) -> Config {
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

pub fn start(config: Config) -> Run {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let session = Session::start(config, move |event| sink.lock().unwrap().push(event));
    Run { session, events }
}

pub fn send(run: &Run, text: &str) {
    run.session
        .send(UserMessage {
            text: text.into(),
            ..UserMessage::default()
        })
        .unwrap();
}

pub fn wait_for(run: &Run, what: &str, ready: impl Fn(&Thread) -> bool) -> Thread {
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

pub fn idle(run: &Run) -> Thread {
    wait_for(run, "the turn's end", |t| {
        t.status() == Status::Idle && !run.session.is_running()
    })
}

pub fn entries(thread: &Thread) -> Vec<Entry> {
    thread.entries(&Lane::Main).iter().map(|e| (**e).clone()).collect()
}

pub fn tool(thread: &Thread, id: &str) -> ide_thread::ToolCall {
    match &thread.entry_by_key(id).expect(id).kind {
        EntryKind::ToolCall(call) => call.clone(),
        other => panic!("{other:?}"),
    }
}

pub fn sent_results(stub: &Stub, request: usize) -> Vec<Value> {
    let body = stub.seen()[request].json();
    let last = body["messages"].as_array().unwrap().last().unwrap().clone();
    last["content"].as_array().unwrap().clone()
}

pub fn notices(thread: &Thread) -> Vec<Notice> {
    entries(thread)
        .iter()
        .filter_map(|e| match &e.kind {
            EntryKind::Notice(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}
