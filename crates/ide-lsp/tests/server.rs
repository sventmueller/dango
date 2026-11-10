//! The client against the fake server in `support/fake_server.rs`, a real process under a real supervisor.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_lsp::lsp_types::{
    ApplyWorkspaceEditResponse, CompletionContext, CompletionTriggerKind, Diagnostic, Position, Range,
    TextDocumentContentChangeEvent, Uri, WorkDoneProgress, WorkspaceEdit,
};
use ide_lsp::{Documents, Encoding, Error, Event, Refresh, RestartPolicy, Server, ServerConfig, Status, uri, wait};
use ide_process::{Ending, Supervisor};
use serde_json::{Value, json};

const WAIT: Duration = Duration::from_secs(10);

/// What the tests look at in the server's events; requests in events are answered on the spot.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    Status(Status),
    Crashed(Option<Ending>),
    Diagnostics(Uri, Vec<Diagnostic>),
    Progress(WorkDoneProgress),
    Log(String),
    ApplyEdit(WorkspaceEdit),
    Capabilities,
    Refresh(Refresh),
    Other,
}

struct Fixture {
    supervisor: Supervisor,
    server: Server,
    seen: Receiver<Seen>,
    root: PathBuf,
}

fn root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-lsp-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(name: &str, env: &[(&str, &str)], tweak: impl FnOnce(&mut ServerConfig)) -> Fixture {
    let supervisor = Supervisor::new();
    let root = root(name);
    let mut env: Vec<(String, String)> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    env.push(("PATH".into(), "/usr/bin:/bin".into()));
    let mut config = ServerConfig::new("fake", Path::new(env!("CARGO_BIN_EXE_ide-lsp-fake-server")), &root, env);
    config.settings = json!({"fake": {"answer": 42}});
    config.restart = RestartPolicy {
        max_restarts: 3,
        first_delay: Duration::from_millis(50),
        max_delay: Duration::from_millis(200),
        stable_after: Duration::from_secs(60),
    };
    config.stop_grace = Duration::from_millis(500);
    tweak(&mut config);
    let (tx, seen) = channel();
    let tx = Mutex::new(tx);
    let server = Server::start(&supervisor, config, move |event| {
        let seen = match event {
            Event::Status(status) => Seen::Status(status),
            Event::Crashed { ending, .. } => Seen::Crashed(ending),
            Event::Diagnostics { uri, diagnostics, .. } => Seen::Diagnostics(uri, diagnostics),
            Event::Progress { progress, .. } => Seen::Progress(progress),
            Event::Log { message, .. } => Seen::Log(message),
            Event::ApplyEdit { edit, reply, .. } => {
                reply.send(ApplyWorkspaceEditResponse {
                    applied: true,
                    failure_reason: None,
                    failed_change: None,
                });
                Seen::ApplyEdit(edit)
            }
            Event::MessageRequest { mut actions, reply, .. } => {
                reply.send(actions.pop());
                Seen::Other
            }
            Event::CapabilitiesChanged => Seen::Capabilities,
            Event::Refresh(kind) => Seen::Refresh(kind),
            _ => Seen::Other,
        };
        let _ = tx.lock().unwrap().send(seen);
    })
    .unwrap();
    assert!(server.wait_until_running(WAIT), "the fake server did not start");
    Fixture {
        supervisor,
        server,
        seen,
        root,
    }
}

fn utf8() -> impl FnOnce(&mut ServerConfig) {
    |config| config.position_encodings = vec![Encoding::Utf16, Encoding::Utf8]
}

impl Fixture {
    fn uri(&self, name: &str) -> Uri {
        uri::from_path(&self.root.join(name))
    }

    fn expect(&self, what: impl Fn(&Seen) -> bool) -> Seen {
        let deadline = Instant::now() + WAIT;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.seen.recv_timeout(left) {
                Ok(seen) if what(&seen) => return seen,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        panic!("the expected event did not come");
    }

    fn ask(&self, method: &str, params: Value) -> Result<Value, Error> {
        wait(|reply| self.server.request_value(method, params, reply))
    }

    fn server_text(&self, uri: &Uri) -> String {
        let answer = self.ask("fake/text", json!({"textDocument": {"uri": uri}})).unwrap();
        answer["text"].as_str().unwrap().to_string()
    }

    fn received(&self) -> Vec<String> {
        serde_json::from_value(self.ask("fake/received", Value::Null).unwrap()).unwrap()
    }
}

fn at(line: u32, character: u32) -> Position {
    Position::new(line, character)
}

fn range(a: (u32, u32), b: (u32, u32)) -> Range {
    Range::new(at(a.0, a.1), at(b.0, b.1))
}

fn edit(r: Option<Range>, text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: r,
        range_length: None,
        text: text.into(),
    }
}

#[test]
fn a_server_counting_utf8_is_answered_and_heard_in_utf16() {
    let f = start("utf8", &[("FAKE_ENCODING", "utf-8")], utf8());
    assert_eq!(f.server.encoding(), Encoding::Utf8);
    let doc = f.uri("main.go");
    f.server.open(&doc, "go", 1, "let ä = 😀word;\nno error here");
    let Seen::Diagnostics(_, diagnostics) =
        f.expect(|s| matches!(s, Seen::Diagnostics(u, d) if *u == doc && !d.is_empty()))
    else {
        unreachable!()
    };
    assert_eq!(diagnostics[0].range, range((1, 3), (1, 8)));
    let hover = wait(|r| f.server.hover(&doc, at(0, 12), r)).unwrap().unwrap();
    assert_eq!(hover.markdown, "`word`");
    assert_eq!(
        hover.range,
        Some(range((0, 10), (0, 14))),
        "😀 is two UTF-16 units and four UTF-8 bytes"
    );
}

#[test]
fn a_server_that_was_offered_only_utf16_counts_in_utf16() {
    let f = start("utf16", &[("FAKE_ENCODING", "utf-8")], |_| {});
    assert_eq!(f.server.encoding(), Encoding::Utf16);
    assert_eq!(f.ask("fake/encoding", Value::Null).unwrap(), "utf-16");
}

#[test]
fn incremental_changes_keep_the_servers_text_equal_to_the_editors() {
    let f = start("sync", &[("FAKE_ENCODING", "utf-8")], utf8());
    let doc = f.uri("a.ts");
    f.server.open(&doc, "typescript", 1, "😀 one\ntwo ä\n");
    f.server
        .change(&doc, 2, &[edit(Some(range((0, 2), (0, 3))), "🎉")])
        .unwrap();
    f.server
        .change(
            &doc,
            3,
            &[
                edit(Some(range((1, 4), (1, 5))), "ö\nthree"),
                edit(Some(range((0, 0), (0, 2))), ""),
            ],
        )
        .unwrap();
    f.server
        .change(&doc, 4, &[edit(Some(range((9, 0), (9, 0))), "end")])
        .unwrap();
    let expected = "🎉one\ntwo ö\nthree\nend";
    assert_eq!(f.server.text(&doc).unwrap(), expected);
    assert_eq!(f.server_text(&doc), expected);
    f.server.change(&doc, 5, &[edit(None, "all new")]).unwrap();
    assert_eq!(f.server_text(&doc), "all new");
    assert_eq!(f.server.change(&f.uri("closed.ts"), 1, &[]), Err(Error::NotOpen));
}

#[test]
fn a_slow_request_times_out_and_a_cancelled_one_says_so_and_both_tell_the_server() {
    let f = start("slow", &[], |config| {
        config.request_timeout = Duration::from_millis(200)
    });
    assert_eq!(f.ask("fake/sleep", json!({"ms": 3000})), Err(Error::Timeout));
    let (tx, rx) = channel();
    let ticket = f
        .server
        .request_value("fake/sleep", json!({"ms": 3000}), move |answer| {
            tx.send(answer).unwrap()
        })
        .unwrap();
    ticket.cancel();
    assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::Cancelled));
    let cancels = f.received().iter().filter(|m| *m == "$/cancelRequest").count();
    assert_eq!(cancels, 2);
}

#[test]
fn requests_from_the_server_are_answered_by_the_client_or_the_app() {
    let f = start("ask", &[("FAKE_ENCODING", "utf-8")], utf8());
    let doc = f.uri("x.rs");
    f.server.open(&doc, "rust", 1, "ä😀x");
    let config = f
        .ask("fake/ask", json!({"method": "workspace/configuration", "params": {"items": [{"section": "fake.answer"}, {"section": "nothing"}]}}))
        .unwrap();
    assert_eq!(config["answer"], json!([42, null]));
    let created = f.ask(
        "fake/ask",
        json!({"method": "window/workDoneProgress/create", "params": {"token": "t"}}),
    );
    assert_eq!(created.unwrap()["answer"], Value::Null);
    let unknown = f
        .ask("fake/ask", json!({"method": "nonsense/thing", "params": {}}))
        .unwrap();
    assert_eq!(unknown["error"]["code"], -32601);
    let message = f
        .ask("fake/ask", json!({"method": "window/showMessageRequest", "params": {"type": 3, "message": "pick", "actions": [{"title": "Yes"}]}}))
        .unwrap();
    assert_eq!(message["answer"]["title"], "Yes");

    let change = json!({"changes": {doc.as_str(): [{"range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 6}}, "newText": "y"}]}});
    let applied = f
        .ask(
            "fake/ask",
            json!({"method": "workspace/applyEdit", "params": {"label": "fix", "edit": change}}),
        )
        .unwrap();
    assert_eq!(applied["answer"]["applied"], true);
    let Seen::ApplyEdit(edit) = f.expect(|s| matches!(s, Seen::ApplyEdit(_))) else {
        unreachable!()
    };
    let edits = &edit.changes.unwrap()[&doc];
    assert_eq!(
        edits[0].range,
        range((0, 1), (0, 3)),
        "UTF-8 bytes 2..6 are ä's end to 😀's end"
    );

    let selection = json!({"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 6}});
    let shown = f
        .ask(
            "fake/ask",
            json!({"method": "window/showDocument", "params": {"uri": doc, "selection": selection}}),
        )
        .unwrap();
    assert_eq!(
        shown["answer"]["success"], false,
        "an unanswered request gets its fallback"
    );
    let refreshed = f
        .ask("fake/ask", json!({"method": "workspace/semanticTokens/refresh"}))
        .unwrap();
    assert_eq!(refreshed["answer"], Value::Null);
    f.expect(|s| *s == Seen::Refresh(Refresh::SemanticTokens));

    let registration = json!({"registrations": [{"id": "w", "method": "workspace/didChangeWatchedFiles", "registerOptions": {"watchers": [{"globPattern": "**/*.rs"}]}}]});
    f.ask(
        "fake/ask",
        json!({"method": "client/registerCapability", "params": registration}),
    )
    .unwrap();
    f.expect(|s| *s == Seen::Capabilities);
    assert_eq!(f.server.watchers()[0]["watchers"][0]["globPattern"], "**/*.rs");
    f.ask("fake/ask", json!({"method": "client/unregisterCapability", "params": {"unregisterations": [{"id": "w", "method": "workspace/didChangeWatchedFiles"}]}}))
        .unwrap();
    assert!(f.server.watchers().is_empty());
}

#[test]
fn progress_and_log_messages_become_events() {
    let f = start("progress", &[], |_| {});
    let begin = json!({"token": "index", "value": {"kind": "begin", "title": "Indexing", "percentage": 0}});
    f.ask("fake/notify", json!({"method": "$/progress", "params": begin}))
        .unwrap();
    let Seen::Progress(WorkDoneProgress::Begin(begin)) = f.expect(|s| matches!(s, Seen::Progress(_))) else {
        panic!("not a begin")
    };
    assert_eq!(begin.title, "Indexing");
    f.ask(
        "fake/notify",
        json!({"method": "window/logMessage", "params": {"type": 4, "message": "hello"}}),
    )
    .unwrap();
    f.expect(|s| *s == Seen::Log("hello".into()));
}

#[test]
fn a_crashed_server_is_restarted_and_gets_its_documents_again() {
    let f = start("crash", &[], |_| {});
    let doc = f.uri("lib.rs");
    f.server.open(&doc, "rust", 1, "fn main() {}");
    f.server
        .change(&doc, 2, &[edit(Some(range((0, 3), (0, 7))), "start")])
        .unwrap();
    let first = f.server.process();
    assert_eq!(f.ask("fake/crash", Value::Null), Err(Error::ServerGone));
    assert_eq!(
        f.expect(|s| matches!(s, Seen::Crashed(_))),
        Seen::Crashed(Some(Ending::Code(3)))
    );
    f.expect(|s| matches!(s, Seen::Status(Status::Restarting { attempt: 1, .. })));
    assert!(f.server.wait_until_running(WAIT));
    assert_ne!(f.server.process(), first);
    assert_eq!(f.server_text(&doc), "fn start() {}");
    assert!(f.received().contains(&"textDocument/didOpen".to_string()));
    let version = f.ask("fake/text", json!({"textDocument": {"uri": doc}})).unwrap()["version"].clone();
    assert_eq!(version, 2, "the document is reopened at its current version");
}

#[test]
fn a_server_that_keeps_crashing_is_given_up_on_until_restarted_by_hand() {
    let supervisor = Supervisor::new();
    let root = root("loop");
    let env = vec![("FAKE_EXIT_AT_START".to_string(), "1".to_string())];
    let mut config = ServerConfig::new("fake", Path::new(env!("CARGO_BIN_EXE_ide-lsp-fake-server")), &root, env);
    config.restart = RestartPolicy {
        max_restarts: 2,
        first_delay: Duration::from_millis(20),
        max_delay: Duration::from_millis(40),
        stable_after: Duration::from_secs(60),
    };
    let crashes = Arc::new(Mutex::new(0));
    let counted = crashes.clone();
    let server = Server::start(&supervisor, config, move |event| {
        if let Event::Crashed { ending, .. } = event {
            assert_eq!(ending, Some(Ending::Code(7)));
            *counted.lock().unwrap() += 1;
        }
    })
    .unwrap();
    assert!(server.wait_for(WAIT, |s| s == Status::Failed));
    let deadline = Instant::now() + WAIT;
    while *crashes.lock().unwrap() < 3 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*crashes.lock().unwrap(), 3, "the first start and two restarts");
    server.restart();
    assert!(server.wait_for(WAIT, |s| s == Status::Starting
        || matches!(s, Status::Restarting { .. })));
    assert!(server.wait_for(WAIT, |s| s == Status::Failed));
}

#[test]
fn stopping_asks_for_shutdown_then_exit_and_the_server_ends_by_itself() {
    let log = root("stoplog").join("log");
    let f = start("stop", &[("FAKE_LOG", log.to_str().unwrap())], |_| {});
    let (tx, rx) = channel();
    let tx = Mutex::new(tx);
    f.supervisor.subscribe(move |event| {
        if let ide_process::Event::Ended { ending, .. } = event {
            let _ = tx.lock().unwrap().send(*ending);
        }
    });
    f.server.stop();
    assert!(f.server.wait_for(WAIT, |s| s == Status::Stopped));
    assert_eq!(
        rx.recv_timeout(WAIT).unwrap(),
        Ending::Code(0),
        "exit after shutdown, no signal needed"
    );
    let methods: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| l.split_once(' ').unwrap().1.to_string())
        .collect();
    assert_eq!(&methods[methods.len() - 2..], ["shutdown", "exit"]);
    assert!(f.supervisor.running().is_empty());
    assert_eq!(f.ask("fake/text", json!({})), Err(Error::NotRunning));
}

#[test]
fn quitting_the_app_stops_the_server_without_a_restart() {
    let f = start("quit", &[], |_| {});
    f.supervisor.shutdown(Duration::from_millis(500));
    assert!(f.supervisor.running().is_empty());
    assert!(f.server.wait_for(WAIT, |s| s == Status::Stopped));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(f.server.status(), Status::Stopped);
}

#[test]
fn dropping_the_last_handle_stops_the_server() {
    let f = start("drop", &[], |_| {});
    let Fixture { supervisor, server, .. } = f;
    let copy = server.clone();
    drop(server);
    assert!(!supervisor.running().is_empty(), "a clone keeps it");
    drop(copy);
    let deadline = Instant::now() + WAIT;
    while !supervisor.running().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(supervisor.running().is_empty());
}

#[test]
fn editor_requests_come_back_in_utf16_and_in_one_shape() {
    let f = start("features", &[("FAKE_ENCODING", "utf-8")], utf8());
    let doc = f.uri("main.rs");
    f.server.open(&doc, "rust", 1, "let 😀 = name;\nlet b = name + name;");
    let context = CompletionContext {
        trigger_kind: CompletionTriggerKind::INVOKED,
        trigger_character: None,
    };
    let completions = wait(|r| f.server.completion(&doc, at(0, 10), Some(context), r)).unwrap();
    assert!(completions.incomplete);
    let first = ide_lsp::insertion(&completions.items[0]);
    assert_eq!(first.snippet.text, "println(x)");
    assert_eq!(first.insert, Some(range((0, 9), (0, 13))));
    let resolved = wait(|r| f.server.resolve_completion(&doc, completions.items[0].clone(), r)).unwrap();
    assert!(resolved.documentation.is_some());

    let definition = wait(|r| f.server.definition(&doc, at(1, 9), r)).unwrap();
    assert_eq!(definition[0].target_selection_range, range((0, 9), (0, 13)));
    let references = wait(|r| f.server.references(&doc, at(1, 9), true, r)).unwrap();
    assert_eq!(references.len(), 3);
    let prepared = wait(|r| f.server.prepare_rename(&doc, at(0, 10), r)).unwrap();
    assert!(prepared.is_some());
    let renamed = wait(|r| f.server.rename(&doc, at(0, 10), "other", r)).unwrap().unwrap();
    assert_eq!(renamed.changes.unwrap()[&doc].len(), 3);

    let symbols = wait(|r| f.server.document_symbols(&doc, r)).unwrap();
    assert_eq!(symbols[0].children.as_ref().unwrap()[0].name, "inner");
    assert_eq!(wait(|r| f.server.folding_ranges(&doc, r)).unwrap().len(), 1);

    let tokens = wait(|r| f.server.semantic_tokens(&doc, r)).unwrap();
    let lets: Vec<(u32, u32, u32)> = tokens.tokens.iter().map(|t| (t.line, t.start, t.length)).collect();
    assert_eq!(lets, [(0, 0, 3), (1, 0, 3)]);
    assert_eq!(tokens.kind(&tokens.tokens[0]), Some("keyword"));
    f.server
        .change(&doc, 2, &[edit(Some(range((0, 0), (0, 0))), "😀let ")])
        .unwrap();
    let again = wait(|r| f.server.semantic_tokens(&doc, r)).unwrap();
    assert_eq!(again.tokens[0].start, 2, "after the emoji, counted in UTF-16");
    assert!(
        f.received()
            .contains(&"textDocument/semanticTokens/full/delta".to_string())
    );

    let pulled = wait(|r| f.server.pull_diagnostics(&doc, r)).unwrap();
    assert_eq!(pulled[0].message, "pulled");
    let unchanged = wait(|r| f.server.pull_diagnostics(&doc, r)).unwrap();
    assert_eq!(unchanged, pulled, "an unchanged report keeps the earlier diagnostics");
}

#[test]
fn what_the_server_does_not_offer_or_does_not_know_is_refused_before_sending() {
    let f = start("refuse", &[], |_| {});
    let doc = f.uri("a.rs");
    let options = ide_lsp::lsp_types::FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..Default::default()
    };
    assert_eq!(
        wait(|r| f.server.format(&doc, options, r)),
        Err(Error::Unsupported("textDocument/formatting"))
    );
    assert_eq!(wait(|r| f.server.hover(&doc, at(0, 0), r)), Err(Error::NotOpen));
    let hints = wait(|r| f.server.inlay_hints(&doc, range((0, 0), (1, 0)), r));
    assert_eq!(hints.err(), Some(Error::Unsupported("textDocument/inlayHint")));
}

#[test]
fn one_document_reaches_every_server_it_is_open_on() {
    let a = start("hub-a", &[("FAKE_ENCODING", "utf-8")], utf8());
    let b = start("hub-b", &[], |_| {});
    let documents = Documents::new();
    let doc = a.uri("shared.ts");
    documents.open(&doc, "typescript", 1, "ä😀", &[a.server.clone(), b.server.clone()]);
    documents
        .change(&doc, 2, &[edit(Some(range((0, 1), (0, 3))), "b")])
        .unwrap();
    assert_eq!(a.server_text(&doc), "äb");
    assert_eq!(b.server_text(&doc), "äb");
    documents.detach(&b.server);
    assert_eq!(documents.servers(&doc).len(), 1);
    documents.close(&doc);
    assert!(!a.server.is_open(&doc));
    assert!(
        b.server.is_open(&doc),
        "a detached server keeps the document until closed on it"
    );
}

#[test]
fn code_actions_signature_help_and_commands_keep_utf16_both_ways() {
    let f = start("actions", &[("FAKE_ENCODING", "utf-8")], utf8());
    let doc = f.uri("a.go");
    f.server.open(&doc, "go", 1, "a😀 error b");
    let Seen::Diagnostics(_, diagnostics) =
        f.expect(|s| matches!(s, Seen::Diagnostics(u, d) if *u == doc && !d.is_empty()))
    else {
        unreachable!()
    };
    assert_eq!(diagnostics[0].range, range((0, 4), (0, 9)));
    let actions = wait(|r| {
        f.server
            .code_actions(&doc, diagnostics[0].range, diagnostics.clone(), None, r)
    })
    .unwrap();
    let ide_lsp::lsp_types::CodeActionOrCommand::CodeAction(action) = actions[0].clone() else {
        panic!("not a code action")
    };
    assert_eq!(
        action.diagnostics.as_ref().unwrap()[0].range,
        diagnostics[0].range,
        "sent as bytes, back as UTF-16"
    );
    let resolved = wait(|r| f.server.resolve_code_action(&doc, action, r)).unwrap();
    assert_eq!(
        resolved.edit.unwrap().changes.unwrap()[&doc][0].range,
        range((0, 4), (0, 9))
    );
    let help = wait(|r| f.server.signature_help(&doc, at(0, 1), None, r))
        .unwrap()
        .unwrap();
    assert_eq!(help.active_parameter, Some(1));
    let edit = json!({"changes": {doc.as_str(): [{"range": {"start": {"line": 0, "character": 1}, "end": {"line": 0, "character": 5}}, "newText": ""}]}});
    wait(|r| f.server.execute_command("fake.apply", vec![edit], r)).unwrap();
    let Seen::ApplyEdit(applied) = f.expect(|s| matches!(s, Seen::ApplyEdit(_))) else {
        unreachable!()
    };
    assert_eq!(applied.changes.unwrap()[&doc][0].range, range((0, 1), (0, 3)));
}
