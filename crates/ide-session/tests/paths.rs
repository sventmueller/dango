//! The driver over the real sessions of each path, against the stand-ins of the session crates and a scripted
//! `claude`: every kind of turn end, requests answered by the user and by an order's rule, the order's launch on the
//! wire, a lost completion event of OpenCode (C62), and the paths that take no role in orders (C48, C72).

mod support;

use std::path::{Path, PathBuf};

use ide_session::rule::UserRule;
use ide_session::{
    AgentPath, Contract, Launch, LaunchError, Order, Outcome, PathKind, Prompt, Registry, Role, SessionId, Signal,
    StreamState, UserChoices, Verdict,
};
use ide_thread::{EntryKind, Lane};
use serde_json::{Value, json};
use support::{
    Harness, claude, failed_with, folder, logged, own_answer, own_config, recorded, stand_in, stand_in_env, waiting,
};

fn user() -> Contract {
    Contract::User(UserChoices::default())
}

fn order(worktree: &Path, build: Option<PathBuf>) -> Order {
    Order {
        order: "o-1".into(),
        role: Role::Builder,
        worktree: worktree.to_path_buf(),
        build_dir: build,
        read_roots: Vec::new(),
        allowed_domains: Vec::new(),
        refuse_outside: true,
        frozen: None,
        repository: None,
        mcp_servers: Vec::new(),
    }
}

fn start(harness: &mut Harness, launch: Launch) -> SessionId {
    let rule = ide_session::rule::for_contract(&launch.contract);
    harness.registry.start(launch, rule).unwrap()
}

fn send(harness: &mut Harness, session: SessionId, prompt: Prompt) -> ide_session::TurnId {
    harness.registry.get_mut(session).unwrap().send(prompt).unwrap()
}

/// Allows every request that waits for the user until `turn` ends.
fn allow_until_end(
    harness: &mut Harness,
    session: SessionId,
    watcher: ide_session::SubscriberId,
    turn: ide_session::TurnId,
) -> Outcome {
    let mut answered = 0;
    loop {
        harness.until(watcher, "a request or the end", |s| {
            support::ended(s, turn).is_some() || waits(s) > answered
        });
        let signals = harness.signals(watcher);
        if let Some(end) = support::ended(&signals, turn) {
            return end.outcome;
        }
        let request = waiting(&signals).unwrap();
        harness
            .registry
            .get_mut(session)
            .unwrap()
            .answer_permission(&request, true, None)
            .unwrap();
        answered += 1;
    }
}

fn waits(signals: &[Signal]) -> usize {
    signals.iter().filter(|s| matches!(s, Signal::Waiting { .. })).count()
}

/// Waits until the agent's answer of the turn that runs shows `text`.
fn shows(harness: &mut Harness, session: SessionId, text: &str) {
    harness.until_state(&format!("the answer {text}"), |r: &Registry| {
        r.get(session)
            .unwrap()
            .thread()
            .entries(&Lane::Main)
            .iter()
            .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown.contains(text)))
    });
}

#[test]
fn codex_ends_each_kind_of_turn_and_the_user_answers_its_requests() {
    let scratch = folder("codex-user");
    let log = scratch.join("sent.ndjson");
    let path = AgentPath::Codex {
        program: stand_in(),
        version: None,
    };
    let mut harness = Harness::new();
    let session = start(
        &mut harness,
        Launch::new(path, &*scratch, stand_in_env("codex", &log), user()),
    );
    let watcher = harness.follow(session);
    let first = send(&mut harness, session, Prompt::new("list the files"));
    assert_eq!(
        allow_until_end(&mut harness, session, watcher, first),
        Outcome::Succeeded
    );
    let failing = send(&mut harness, session, Prompt::new("please fail"));
    let outcome = harness.end_of(watcher, failing).outcome;
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    let slow = send(&mut harness, session, Prompt::new("slow please"));
    shows(&mut harness, session, "Working");
    harness.registry.get_mut(session).unwrap().stop_turn();
    assert_eq!(harness.end_of(watcher, slow).outcome, Outcome::Cancelled);
    let crash = send(&mut harness, session, Prompt::new("crash now"));
    assert_eq!(harness.end_of(watcher, crash).outcome, Outcome::ConnectionLost);
    assert!(logged(&log).iter().any(|l| l["method"] == "turn/start"));
}

#[test]
fn a_codex_order_runs_on_a_server_of_its_own_with_no_approvals_and_its_rule_refuses_what_asks() {
    let scratch = folder("codex-order");
    let log = scratch.join("sent.ndjson");
    let build = scratch.join("build");
    let path = AgentPath::Codex {
        program: stand_in(),
        version: None,
    };
    let contract = Contract::Order(order(&scratch, Some(build.clone())));
    let mut harness = Harness::new();
    let session = start(
        &mut harness,
        Launch::new(path, &*scratch, stand_in_env("codex", &log), contract),
    );
    let watcher = harness.follow(session);
    let first = send(&mut harness, session, Prompt::new("list the files"));
    assert_eq!(harness.end_of(watcher, first).outcome, Outcome::Succeeded);
    let denied = harness
        .signals(watcher)
        .iter()
        .filter(|s| {
            matches!(
                s,
                Signal::Decided {
                    verdict: Verdict::Deny(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(denied, 2, "the command and the edit asked, and the order refused both");
    let mut prompt = Prompt::new("answer as JSON");
    prompt.schema = Some(json!({"type": "object", "properties": {"verdict": {"type": "string"}}}));
    let second = send(&mut harness, session, prompt);
    harness.end_of(watcher, second);
    let sent = logged(&log);
    let opened = sent.iter().find(|l| l["method"] == "thread/start").unwrap();
    assert_eq!(opened["params"]["approvalPolicy"], "never");
    assert_eq!(opened["params"]["sandbox"], "workspace-write");
    let turns: Vec<&Value> = sent.iter().filter(|l| l["method"] == "turn/start").collect();
    assert_eq!(turns.len(), 2);
    for turn in &turns {
        assert_eq!(turn["params"]["approvalPolicy"], "never");
        assert_eq!(
            turn["params"]["sandboxPolicy"],
            json!({"type": "workspaceWrite", "writableRoots": [build.to_string_lossy()], "networkAccess": false,
                   "excludeTmpdirEnvVar": true, "excludeSlashTmp": true})
        );
    }
    assert!(turns[0]["params"].get("outputSchema").is_none());
    assert_eq!(turns[1]["params"]["outputSchema"]["type"], "object");
}

#[test]
fn opencode_reports_a_lost_completion_event_ends_the_users_turn_and_reads_the_session_again() {
    let scratch = folder("opencode-gap");
    let log = scratch.join("sent.ndjson");
    let path = AgentPath::OpenCode {
        program: stand_in(),
        version: None,
    };
    let mut harness = Harness::new();
    let session = start(
        &mut harness,
        Launch::new(path, &*scratch, stand_in_env("opencode", &log), user()),
    );
    let watcher = harness.follow(session);
    let gap = send(&mut harness, session, Prompt::new("mind the gap"));
    let end = harness.end_of(watcher, gap);
    assert_eq!(end.outcome, Outcome::Succeeded);
    // Nothing holds the turn's end for the gap, so the server may be read again only after it.
    harness.until(watcher, "the session was read again", |s| s.contains(&Signal::Resynced));
    let signals = harness.signals(watcher);
    let at = |wanted: &Signal| signals.iter().position(|s| s == wanted);
    let lost = at(&Signal::StreamGap).expect("the gap is reported");
    let caught_up = at(&Signal::Resynced).expect("the session was read again");
    assert!(lost < caught_up, "{signals:?}");
    let driver = harness.registry.get(session).unwrap();
    assert_eq!(*driver.stream(), StreamState::Live);
    let answered =
        driver.thread().entries(&Lane::Main).iter().any(
            |e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown == ide_opencode::test_server::GAP_ANSWER),
        );
    assert!(answered);
    let failing = send(&mut harness, session, Prompt::new("fail now"));
    assert!(failed_with(
        &harness.end_of(watcher, failing).outcome,
        "Invalid API key"
    ));
    let crash = send(&mut harness, session, Prompt::new("crash please"));
    assert_eq!(harness.end_of(watcher, crash).outcome, Outcome::ConnectionLost);
}

#[test]
fn opencode_acp_and_paths_without_an_allow_list_take_no_order_and_start_nothing() {
    let scratch = folder("refused");
    let mut registry = Harness::new().registry;
    let refused = |registry: &mut Registry, path: AgentPath, order: Order| {
        let contract = Contract::Order(order);
        let rule = ide_session::rule::for_contract(&contract);
        registry
            .start(Launch::new(path, &*scratch, Vec::new(), contract), rule)
            .err()
    };
    let missing = PathBuf::from("/nonexistent/agent");
    let opencode = AgentPath::OpenCode {
        program: missing.clone(),
        version: None,
    };
    assert_eq!(
        refused(&mut registry, opencode, order(&scratch, None)),
        Some(LaunchError::NoRole(PathKind::OpenCode))
    );
    let acp = ide_acp::Launch::new("gemini", "Gemini CLI", &missing, Vec::new(), Vec::new(), &*scratch);
    assert_eq!(
        refused(&mut registry, AgentPath::Acp(Box::new(acp)), order(&scratch, None)),
        Some(LaunchError::NoRole(PathKind::Acp))
    );
    let mut online = order(&scratch, None);
    online.allowed_domains = vec!["crates.io".into()];
    let codex = AgentPath::Codex {
        program: missing.clone(),
        version: None,
    };
    assert!(matches!(
        refused(&mut registry, codex, online),
        Some(LaunchError::Unsupported {
            path: PathKind::Codex,
            ..
        })
    ));
    let elsewhere = order(&scratch.join("other"), None);
    let claude = AgentPath::claude(missing);
    assert!(matches!(
        refused(&mut registry, claude, elsewhere),
        Some(LaunchError::Folder { .. })
    ));
    assert!(registry.is_empty());
}

fn claude_turn(name: &str, frames: Vec<String>, after: Vec<String>, exit: bool, stop: bool) -> Outcome {
    let scratch = folder(name);
    let (path, env) = claude(&scratch, &frames, &after, exit);
    let mut harness = Harness::new();
    let session = start(&mut harness, Launch::new(path, &*scratch, env, user()));
    let watcher = harness.follow(session);
    let turn = send(&mut harness, session, Prompt::new("Tell me a story"));
    if stop {
        shows(&mut harness, session, "Once upon a time");
        harness.registry.get_mut(session).unwrap().stop_turn();
    }
    harness.end_of(watcher, turn).outcome
}

#[test]
fn claude_ends_each_kind_of_turn() {
    let succeeded = claude_turn(
        "claude-ok",
        recorded("api_retry.ndjson", None),
        Vec::new(),
        false,
        false,
    );
    assert_eq!(succeeded, Outcome::Succeeded);
    let signed_out = recorded("recorded_signed_out.ndjson", None);
    let failed = claude_turn("claude-failed", signed_out, Vec::new(), false, false);
    assert!(failed_with(&failed, "/login"), "{failed:?}");
    let story = recorded("interrupt.ndjson", Some(r#""subtype":"interrupt""#));
    let result: Vec<String> = recorded("interrupt.ndjson", None)
        .into_iter()
        .filter(|l| l.contains(r#""type":"result""#))
        .collect();
    let cancelled = claude_turn("claude-cancelled", story.clone(), result, false, true);
    assert_eq!(cancelled, Outcome::Cancelled);
    let lost = claude_turn("claude-lost", story, Vec::new(), true, false);
    assert_eq!(lost, Outcome::ConnectionLost);
}

#[test]
fn a_claude_order_starts_without_questions_mcp_or_bypass_and_with_its_sandbox() {
    let scratch = folder("claude-order");
    let frames = recorded("api_retry.ndjson", None);
    let (path, env) = claude(&scratch, &frames, &[], false);
    let mut harness = Harness::new();
    let contract = Contract::Order(order(&scratch, Some(scratch.join("build"))));
    let session = start(&mut harness, Launch::new(path, &*scratch, env, contract));
    let watcher = harness.follow(session);
    let turn = send(&mut harness, session, Prompt::new("Build it"));
    assert_eq!(harness.end_of(watcher, turn).outcome, Outcome::Succeeded);
    let args = std::fs::read_to_string(scratch.join("args.txt")).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert!(args.contains(&"--permission-mode=dontAsk"), "{args:?}");
    assert!(args.contains(&"--strict-mcp-config"));
    assert!(!args.iter().any(|a| a.contains("dangerously")));
    let settings = args.iter().find_map(|a| a.strip_prefix("--settings=")).unwrap();
    let settings: Value = serde_json::from_str(settings).unwrap();
    assert_eq!(settings["sandbox"]["allowUnsandboxedCommands"], false);
    assert!(
        args.iter()
            .any(|a| a.starts_with("--disallowedTools=") && a.contains("Bash(git push:*)"))
    );
}

#[test]
fn the_own_agent_works_on_an_order_under_the_orders_policy_and_ends_its_turns_typed() {
    use ide_agent::stub::{Answer, Stub};
    let scratch = folder("own");
    let project = scratch.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let refused = json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}});
    let stub = Stub::script(vec![Answer::events(&own_answer("Done.")), Answer::json(401, &refused)]).unwrap();
    let config = own_config(&project, &stub.url(), None);
    let mut elsewhere = order(&scratch, None);
    elsewhere.order = "o-2".into();
    let mut harness = Harness::new();
    let contract = Contract::Order(elsewhere);
    let launch = Launch::new(
        AgentPath::Own(Box::new(config.clone())),
        &*scratch,
        Vec::new(),
        contract.clone(),
    );
    let rule = ide_session::rule::for_contract(&contract);
    assert!(matches!(
        harness.registry.start(launch, rule),
        Err(LaunchError::Folder { .. })
    ));
    let contract = Contract::Order(order(&project, Some(scratch.join("build"))));
    let launch = Launch::new(AgentPath::Own(Box::new(config)), &project, Vec::new(), contract);
    let session = start(&mut harness, launch);
    let watcher = harness.follow(session);
    let first = send(&mut harness, session, Prompt::new("Build it"));
    assert_eq!(harness.end_of(watcher, first).outcome, Outcome::Succeeded);
    assert_eq!(
        harness.registry.get(session).unwrap().thread().mode(),
        Some(ide_agent::session::mode_id(ide_agent::Approval::All))
    );
    let second = send(&mut harness, session, Prompt::new("Again"));
    assert!(matches!(harness.end_of(watcher, second).outcome, Outcome::Failed(_)));
    let _ = UserRule;
}
