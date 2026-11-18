//! The agent's turn against a stub provider: attachments and settings in the request, a busy or stopped session,
//! the length limit, refusals, failed requests, broken tool input, a model fallback and a store that cannot be written.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{Project, config, entries, event, idle, notices, says, send, sent_results, start, tool};
use ide_agent::stub::{Answer, Stub};
use ide_agent::{Approval, Choice, Event, Profile, SessionError};
use ide_thread::{EntryKind, Notice, ToolStatus, UserMessage};
use serde_json::{Value, json};

/// An Anthropic stream of `blocks`, each a content block and its deltas, that stops for `stop` with `category`.
fn stream(blocks: &[(Value, Vec<Value>)], stop: &str, category: Option<&str>) -> Answer {
    let mut text = event(
        "message_start",
        &json!({"type": "message_start", "message": {"model": "claude-opus-5-5", "usage": {"input_tokens": 50, "output_tokens": 1}}}),
    );
    for (index, (block, deltas)) in blocks.iter().enumerate() {
        text.push_str(&event(
            "content_block_start",
            &json!({"type": "content_block_start", "index": index, "content_block": block}),
        ));
        for delta in deltas {
            text.push_str(&event(
                "content_block_delta",
                &json!({"type": "content_block_delta", "index": index, "delta": delta}),
            ));
        }
        text.push_str(&event(
            "content_block_stop",
            &json!({"type": "content_block_stop", "index": index}),
        ));
    }
    text.push_str(&event(
        "message_delta",
        &json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_details": {"category": category}}, "usage": {"output_tokens": 30}}),
    ));
    text.push_str(&event("message_stop", &json!({"type": "message_stop"})));
    Answer::events(&text)
}

fn text_block(text: &str) -> (Value, Vec<Value>) {
    (
        json!({"type": "text", "text": ""}),
        vec![json!({"type": "text_delta", "text": text})],
    )
}

fn tool_block(id: &str, name: &str, partial_json: &str) -> (Value, Vec<Value>) {
    (
        json!({"type": "tool_use", "id": id, "name": name, "input": {}}),
        vec![json!({"type": "input_json_delta", "partial_json": partial_json})],
    )
}

#[test]
fn attachments_and_changed_settings_reach_the_next_request() {
    let project = Project::new("attachments");
    let stub = Stub::script(vec![says("Seen.")]).unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    run.session.set_model("claude-sonnet-5", None, Some(200_000));
    run.session.set_effort(None);
    run.session.set_approval(Approval::All);
    run.session.set_sandbox(true);
    run.session.set_profile(Profile::Ask);
    let policy = run.session.policy();
    assert_eq!(
        (policy.approval, policy.sandbox, policy.profile),
        (Approval::All, true, Profile::Ask)
    );
    let main = project.root().join("src/main.rs");
    run.session
        .send(UserMessage {
            text: "Look at these".into(),
            attachments: vec![
                ide_thread::Attachment::File {
                    path: main.clone(),
                    lines: Some((2, 2)),
                },
                ide_thread::Attachment::File {
                    path: project.root().join("missing.rs"),
                    lines: None,
                },
                ide_thread::Attachment::Selection {
                    path: main.clone(),
                    lines: (1, 1),
                    text: "fn main() {".into(),
                },
                ide_thread::Attachment::External {
                    label: "zsh".into(),
                    source: "the output of the terminal \"zsh\"".into(),
                    text: "ok".into(),
                },
                ide_thread::Attachment::Image {
                    name: None,
                    media_type: "image/png".into(),
                    data: Arc::from(&b"\x89PNG"[..]),
                },
            ],
            ..UserMessage::default()
        })
        .unwrap();
    let thread = idle(&run);
    assert_eq!(
        (thread.mode(), thread.model(), thread.effort()),
        (Some("all"), Some("claude-sonnet-5"), None)
    );

    let body = stub.seen()[0].json();
    assert_eq!(body["model"], "claude-sonnet-5");
    assert!(body.get("output_config").is_none(), "no effort is sent: {body}");
    let tools: Vec<&str> = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        !tools.contains(&"write_file"),
        "the read profile offers no writing tool: {tools:?}"
    );
    let content = body["messages"][0]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    assert_eq!(
        text,
        format!(
            "Look at these\n\n<file path=\"src/main.rs\">\n    println!(\"hello\");\n</file>\n\n<selection path=\"src/main.rs\" lines=\"1-1\">\nfn main() {{\n</selection>\n\n{}",
            ide_thread::external_text("the output of the terminal \"zsh\"", "ok")
        ),
        "a missing file is left out"
    );
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["data"], "iVBORw==");
}

#[test]
fn a_session_refuses_a_second_turn_while_busy_and_everything_once_stopped() {
    let project = Project::new("busy");
    let slow = says("Done.");
    let stub = Stub::script(vec![Answer {
        parts: vec![(Duration::from_millis(400), slow.parts[0].1.clone())],
        ..slow
    }])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "first");
    let busy = run.session.send(UserMessage {
        text: "second".into(),
        ..UserMessage::default()
    });
    assert_eq!(busy, Err(SessionError::Busy));
    assert_eq!(run.session.compact(None), Err(SessionError::Busy));
    assert_eq!(
        run.session.answer("p:none", Choice::Dismiss),
        Err(SessionError::NoRequest)
    );
    idle(&run);
    assert_eq!(run.session.with_thread(|t| t.title().to_string()), "first");
    run.session.stop();
    let closed = run.session.send(UserMessage::default());
    assert_eq!(closed, Err(SessionError::Closed));
    assert_eq!(
        [SessionError::Busy, SessionError::NoRequest, SessionError::Closed].map(|e| e.to_string()),
        [
            "the agent is still working",
            "no such request is waiting",
            "the session has ended"
        ]
    );
}

#[test]
fn a_reply_cut_off_by_the_length_limit_says_so_and_cut_off_calls_are_answered() {
    let project = Project::new("length");
    let stub = Stub::script(vec![
        stream(
            &[
                (
                    json!({"type": "thinking", "thinking": ""}),
                    vec![json!({"type": "thinking_delta", "thinking": "Let me write it."})],
                ),
                tool_block("w1", "write_file", "{\"path\": \"big.txt\", \"content\": \"abc"),
            ],
            "max_tokens",
            None,
        ),
        stream(&[text_block("I ran out of room")], "max_tokens", None),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::All, false, None));
    send(&run, "Write a big file");
    let thread = idle(&run);
    assert!(!project.root().join("big.txt").exists(), "a cut-off call never runs");
    assert_eq!(tool(&thread, "w1").status, ToolStatus::Failed);
    let results = sent_results(&stub, 1);
    assert_eq!(results[0]["tool_use_id"], "w1");
    assert_eq!(results[0]["is_error"], true);
    assert!(
        results[0]["content"].as_str().unwrap().contains("length limit"),
        "{results:?}"
    );
    let thoughts: Vec<_> = entries(&thread)
        .into_iter()
        .filter_map(|e| match e.kind {
            EntryKind::Thought(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(thoughts.len(), 1);
    assert!(!thoughts[0].streaming && thoughts[0].text == "Let me write it.");
    assert!(
        notices(&thread)
            .iter()
            .any(|n| matches!(n, Notice::Info { text, .. } if text == "LengthLimit")),
        "{:?}",
        notices(&thread)
    );
}

#[test]
fn a_refusal_and_a_failed_request_end_the_turn_with_an_error() {
    let project = Project::new("refusal");
    let stub = Stub::script(vec![
        stream(&[text_block("I can't")], "refusal", Some("cyber")),
        Answer::json(
            401,
            &json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
        ),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::Manual, false, None));
    send(&run, "first");
    let thread = idle(&run);
    assert!(
        matches!(notices(&thread).last(), Some(Notice::Error { kind: ide_thread::ErrorKind::Api, message }) if message == "Refused(Some(\"cyber\"))"),
        "{:?}",
        notices(&thread)
    );
    send(&run, "second");
    let thread = idle(&run);
    assert!(
        matches!(
            notices(&thread).last(),
            Some(Notice::Error {
                kind: ide_thread::ErrorKind::Auth,
                ..
            })
        ),
        "{:?}",
        notices(&thread)
    );
    assert_eq!(stub.seen().len(), 2, "an authentication error is not retried");
}

#[test]
fn a_call_with_broken_input_is_answered_without_running_and_a_fallback_drops_the_first_models_calls() {
    let project = Project::new("broken");
    let stub = Stub::script(vec![
        stream(
            &[
                tool_block("x1", "read_file", "{\"path\": \"src/main.rs\""),
                (
                    json!({"type": "fallback", "from": {"model": "claude-opus-5-5"}, "to": {"model": "claude-sonnet-5"}}),
                    vec![],
                ),
                text_block("Answered by the other model."),
            ],
            "end_turn",
            None,
        ),
        stream(&[tool_block("x2", "read_file", "[1,")], "tool_use", None),
        says("Fine."),
    ])
    .unwrap();
    let run = start(config(&stub, &project, Approval::All, false, None));
    send(&run, "first");
    let thread = idle(&run);
    assert!(
        thread.entry_by_key("x1").is_none(),
        "the declined model's call is taken back"
    );
    assert!(
        notices(&thread)
            .iter()
            .any(|n| matches!(n, Notice::Info { text, .. } if text.starts_with("Fallback"))),
        "{:?}",
        notices(&thread)
    );
    send(&run, "second");
    let thread = idle(&run);
    assert_eq!(tool(&thread, "x2").status, ToolStatus::Failed);
    let results = sent_results(&stub, 2);
    assert_eq!(results[0]["is_error"], true);
    assert_eq!(results[0]["content"], json!({"INVALID_JSON": "[1,"}).to_string());
}

#[test]
fn a_conversation_that_cannot_be_stored_is_reported() {
    let project = Project::new("unstored");
    let blocker = project.0.join("blocker");
    std::fs::write(&blocker, "a file where a folder should be").unwrap();
    let stub = Stub::script(vec![says("Hi.")]).unwrap();
    let run = start(config(
        &stub,
        &project,
        Approval::Manual,
        false,
        Some(blocker.join("t1.json")),
    ));
    send(&run, "hi");
    idle(&run);
    let failed: Vec<String> = run
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Failed(why) => Some(why.clone()),
            _ => None,
        })
        .collect();
    assert!(!failed.is_empty() && failed[0].contains("blocker"), "{failed:?}");
}
