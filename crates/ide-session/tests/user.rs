//! What the driver offers the user's session beyond an order's (B37b), against the stand-ins of each path: the
//! message's id and the checkpoint a prompt went out after, the checkpoint's end, Claude's answers with an edited
//! input and rules to keep, answers by any option a request offers, messages into the running turn, Codex's shared
//! server and its Remote Control, a rewind at launch, the typed failure of each path, and the own agent continuing a
//! stored conversation.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

mod support;

use ide_claude::PermissionAnswer;
use ide_session::{
    AgentPath, AnswerError, Contract, Driver, Launch, Outcome, Prompt, Registry, SessionId, Signal, SubscriberId,
    TurnId, UserChoices, UserRule,
};
use ide_thread::{EntryKind, Lane, PermissionOption, RuleScope, UserMessage};
use serde_json::json;
use support::{
    Harness, claude, claude_args, claude_read, folder, git_env, logged, own_answer, own_config, recorded, stand_in,
    stand_in_env, waiting,
};

fn user() -> Contract {
    Contract::User(UserChoices::default())
}

/// Starts the user's session `launch` describes and follows it.
fn start(harness: &mut Harness, launch: Launch) -> (SessionId, SubscriberId) {
    let session = harness.registry.start(launch, Box::new(UserRule)).unwrap();
    let watcher = harness.follow(session);
    (session, watcher)
}

fn send(harness: &mut Harness, session: SessionId, text: &str) -> TurnId {
    harness
        .registry
        .get_mut(session)
        .unwrap()
        .send(Prompt::new(text))
        .unwrap()
}

fn message(text: &str) -> UserMessage {
    UserMessage {
        text: text.into(),
        ..UserMessage::default()
    }
}

/// Waits until `watcher` heard of `count` requests left to the user; the id of the last.
fn nth_request(harness: &mut Harness, watcher: SubscriberId, count: usize) -> String {
    harness.until(watcher, "a request for the user", |s| {
        s.iter().filter(|s| matches!(s, Signal::Waiting { .. })).count() >= count
    });
    waiting(&harness.signals(watcher)).unwrap()
}

/// Where the permission request `request` offers the option `wanted` holds for.
fn offered(driver: &Driver, request: &str, wanted: impl Fn(&PermissionOption) -> bool) -> usize {
    (driver.thread().pending_requests().into_iter())
        .find_map(|e| match &e.kind {
            EntryKind::PermissionRequest(r) if r.request_id == request => r.options.iter().position(&wanted),
            _ => None,
        })
        .expect("the request offers the option")
}

/// Whether an answer of the agent in `session` holds `text`.
fn says(registry: &Registry, session: SessionId, text: &str) -> bool {
    let thread = registry.get(session).unwrap().thread();
    (thread.entries(&Lane::Main).iter())
        .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown.contains(text)))
}

/// The signals of a user's session `launch` describes until one `done` holds for came.
fn heard_until(launch: Launch, done: impl Fn(&Signal) -> bool) -> Vec<Signal> {
    let mut harness = Harness::new();
    let (_, watcher) = start(&mut harness, launch);
    harness.until(watcher, "the typed failure", |s| s.iter().any(&done));
    harness.signals(watcher)
}

/// Asserts that a failure in the path's words came before the typed one `typed` holds for.
fn typed_after_words(signals: &[Signal], typed: impl Fn(&Signal) -> bool) {
    let words = signals.iter().position(|s| matches!(s, Signal::Failed(_)));
    let typed = signals.iter().position(typed);
    assert!(matches!((words, typed), (Some(w), Some(t)) if w < t), "{signals:?}");
}

#[test]
fn claude_hands_on_the_message_id_and_the_checkpoint_and_records_the_turn_s_end_in_it() {
    let scratch = folder("claude-delivered");
    let worktree = scratch.join("wt");
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::create_dir_all(scratch.join("home")).unwrap();
    let git = ide_git::Git::new(&ide_process::Supervisor::new(), git_env(&scratch.join("home")));
    git.init(&worktree).unwrap();
    let (path, env) = claude(&scratch, &recorded("api_retry.ndjson", None), &[], false);
    let mut launch = Launch::new(path, &worktree, env, user());
    launch.git = Some(git);
    let mut harness = Harness::new();
    let (session, watcher) = start(&mut harness, launch);
    let turn = send(&mut harness, session, "Fix the build\nand test it");
    let end = harness.end_of(watcher, turn);
    assert_eq!(end.outcome, Outcome::Succeeded);
    let delivered = harness.signals(watcher).into_iter().find_map(|s| match s {
        Signal::Delivered {
            turn: t,
            sent: Ok(Some(id)),
            checkpoint: Some(checkpoint),
        } if t == turn => Some((id, checkpoint)),
        _ => None,
    });
    let (id, checkpoint) = delivered.expect("the prompt went out with its id, after its checkpoint");
    assert_eq!(checkpoint.label, "Fix the build…", "labelled as the AI tab labels it");
    assert_eq!(end.checkpoint.as_ref(), Some(&checkpoint));
    harness.until_state("the message on the wire", |_| claude_read(&scratch).contains(&id));
    harness.until(watcher, "the turn's end in its checkpoint", |s| {
        s.iter()
            .any(|s| matches!(s, Signal::CheckpointFinished { turn: t, .. } if *t == turn))
    });
    let started = harness.signals(watcher).into_iter().find_map(|s| match s {
        Signal::CheckpointFinished { turn: t, started, .. } if t == turn => Some(started),
        _ => None,
    });
    assert_eq!(started, Some(checkpoint.commit));
}

/// The signals of a Claude session that plays `frames` after its prompt, resuming `s-old` at a rewind when `rewind`,
/// until a typed failure came; and the arguments `claude` was started with.
fn claude_failure(name: &str, frames: Vec<String>, rewind: bool) -> (Vec<Signal>, Vec<String>) {
    let scratch = folder(name);
    let (path, env) = claude(&scratch, &frames, &[], false);
    let AgentPath::Claude { program, .. } = path else {
        unreachable!("the stand-in is a claude")
    };
    let rewind = rewind.then(|| ide_thread::Rewind {
        fork_at: "m-kept".into(),
        drops_turn: "p-dropped".into(),
    });
    let resume = rewind.as_ref().map(|_| "s-old".to_string());
    let path = AgentPath::Claude {
        program,
        remote_control: None,
        rewind,
    };
    let mut launch = Launch::new(path, &*scratch, env, user());
    launch.resume = resume;
    let mut harness = Harness::new();
    let (session, watcher) = start(&mut harness, launch);
    send(&mut harness, session, "Reply with the single word three.");
    harness.until(watcher, "Claude's typed failure", |s| {
        s.iter().any(|s| matches!(s, Signal::ClaudeFailed(_)))
    });
    (harness.signals(watcher), claude_args(&scratch))
}

#[test]
fn claude_rewinds_at_launch_and_reports_a_refused_rewind_and_a_missing_sign_in_typed() {
    let (signals, args) = claude_failure("claude-rewind", recorded("recorded_rewind_rejected.ndjson", None), true);
    for wanted in [
        "--resume=s-old",
        "--resume-session-at=m-kept",
        "--resume-drops-turn=p-dropped",
        "--fork-session",
    ] {
        assert!(args.iter().any(|a| a == wanted), "{wanted} in {args:?}");
    }
    typed_after_words(&signals, |s| {
        matches!(s, Signal::ClaudeFailed(ide_claude::SessionError::RewindRejected(_)))
    });
    let (signals, args) = claude_failure("claude-signed-out", recorded("recorded_signed_out.ndjson", None), false);
    assert!(!args.iter().any(|a| a.starts_with("--resume")), "{args:?}");
    typed_after_words(&signals, |s| {
        matches!(s, Signal::ClaudeFailed(ide_claude::SessionError::SignInRequired(_)))
    });
}

#[test]
fn claude_takes_an_edited_input_and_allows_destructive_git_once_but_never_for_good() {
    let scratch = folder("claude-permission");
    let ask = json!({"type": "control_request", "request_id": "perm-rm", "request": {"subtype": "can_use_tool",
        "tool_name": "Bash", "input": {"command": "git clean -fd", "description": "Remove untracked files"},
        "permission_suggestions": [{"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "git clean:*"}],
                                    "behavior": "allow", "destination": "localSettings"}],
        "tool_use_id": "toolu_rm"}});
    let (path, env) = claude(&scratch, &[ask.to_string()], &[], false);
    let mut harness = Harness::new();
    let (session, watcher) = start(&mut harness, Launch::new(path, &*scratch, env, user()));
    send(&mut harness, session, "Clean up");
    assert_eq!(nth_request(&mut harness, watcher, 1), "perm-rm");
    let driver = harness.registry.get_mut(session).unwrap();
    let for_good = PermissionAnswer::Allow {
        updated_input: None,
        always: vec![0],
    };
    assert!(matches!(
        driver.answer_permission_as("perm-rm", for_good),
        Err(AnswerError::Path(_))
    ));
    let elsewhere = PermissionAnswer::Allow {
        updated_input: None,
        always: Vec::new(),
    };
    assert!(matches!(
        driver.answer_permission_as("perm-none", elsewhere),
        Err(AnswerError::NoRequest(_))
    ));
    let edited = PermissionAnswer::Allow {
        updated_input: Some(json!({"command": "git clean -n", "description": "Show what would go"})),
        always: Vec::new(),
    };
    driver.answer_permission_as("perm-rm", edited).unwrap();
    harness.until_state("the edited input on the wire", |_| {
        let read = claude_read(&scratch);
        read.contains("perm-rm") && read.contains("git clean -n")
    });
    assert!(!claude_read(&scratch).contains("updatedPermissions"), "no rule kept");
}

#[test]
fn a_users_codex_session_takes_any_option_offered_no_rule_and_messages_into_its_running_turn() {
    let scratch = folder("codex-user-calls");
    let log = scratch.join("sent.ndjson");
    let path = AgentPath::Codex {
        program: stand_in(),
        version: None,
    };
    let mut harness = Harness::new();
    let launch = Launch::new(path, &*scratch, stand_in_env("codex", &log), user());
    let (session, watcher) = start(&mut harness, launch);
    let turn = send(&mut harness, session, "list the files");
    let command = nth_request(&mut harness, watcher, 1);
    let driver = harness.registry.get_mut(session).unwrap();
    let for_the_session = offered(driver, &command, |o| {
        matches!(
            o,
            PermissionOption::AllowAlways {
                scope: RuleScope::Session,
                ..
            }
        )
    });
    driver.answer_option(&command, Some(for_the_session)).unwrap();
    let file = nth_request(&mut harness, watcher, 2);
    let driver = harness.registry.get_mut(session).unwrap();
    let edited = PermissionAnswer::Allow {
        updated_input: Some(json!({})),
        always: Vec::new(),
    };
    assert!(
        matches!(driver.answer_permission_as(&file, edited), Err(AnswerError::Path(_))),
        "only Claude takes an edited input"
    );
    let refused = PermissionAnswer::Deny {
        message: "Not this file".into(),
        interrupt: false,
    };
    driver.answer_permission_as(&file, refused).unwrap();
    assert_eq!(harness.end_of(watcher, turn).outcome, Outcome::Succeeded);
    let decision = |id: u64| {
        (logged(&log).into_iter())
            .find(|l| l["id"] == id && l.get("result").is_some())
            .map(|l| l["result"]["decision"].clone())
    };
    assert_eq!(decision(900), Some(json!("acceptForSession")));
    assert_eq!(decision(901), Some(json!("decline")));

    let slow = send(&mut harness, session, "slow please");
    harness.until_state("the running turn", |r| says(r, session, "Working"));
    let driver = harness.registry.get_mut(session).unwrap();
    assert_eq!(driver.steer(message("and mind the tests")), Ok(None));
    assert_eq!(driver.queued(), 0, "it went into the running turn, not into the queue");
    harness.until_state("the message in the running turn", |_| {
        (logged(&log).iter()).any(|l| l["method"] == "turn/steer" && l.to_string().contains("mind the tests"))
    });
    harness.registry.get_mut(session).unwrap().stop_turn();
    assert_eq!(harness.end_of(watcher, slow).outcome, Outcome::Cancelled);
}

#[test]
fn a_users_codex_session_joins_the_shared_server_when_asked_and_tells_where_remote_control_stands() {
    let scratch = folder("codex-shared");
    let log = scratch.join("sent.ndjson");
    let mut env = stand_in_env("codex", &log);
    let home = scratch.join("codex-home");
    env.push(("CODEX_HOME".into(), home.to_string_lossy().into_owned()));
    ide_codex::remote::start(&ide_process::Supervisor::new(), &stand_in(), &env).unwrap();
    let contract = Contract::User(UserChoices {
        shared: true,
        ..UserChoices::default()
    });
    let path = AgentPath::Codex {
        program: stand_in(),
        version: None,
    };
    let mut harness = Harness::new();
    let (session, watcher) = start(&mut harness, Launch::new(path, &*scratch, env, contract));
    harness.until(watcher, "Remote Control", |s| s.contains(&Signal::RemoteControl));
    let codex = harness.registry.get(session).unwrap().codex().cloned().unwrap();
    let connected = ide_codex::RemoteControl::Connected(ide_codex::test_server::SERVER_NAME.into());
    harness.until_state("the phone's connection", |_| codex.remote_control() == connected);
    let shared = (logged(&log).iter())
        .any(|l| l["method"] == "initialize" && l["params"]["capabilities"]["experimentalApi"] == true);
    assert!(shared, "the session went through the shared server");
}

#[test]
fn every_path_follows_its_failure_in_words_with_the_typed_error() {
    let scratch = folder("typed-failures");
    let mut env = stand_in_env("codex", &scratch.join("codex.ndjson"));
    env.push(("CODEX_TEST_SCENARIO".into(), "signin".into()));
    let codex = AgentPath::Codex {
        program: stand_in(),
        version: None,
    };
    let signals = heard_until(Launch::new(codex, &*scratch, env, user()), |s| {
        matches!(s, Signal::CodexFailed(_))
    });
    typed_after_words(&signals, |s| {
        matches!(s, Signal::CodexFailed(ide_codex::SessionError::SignIn))
    });

    let mut env = stand_in_env("opencode", &scratch.join("opencode.ndjson"));
    env.push((ide_opencode::test_server::SCENARIO_VARIABLE.into(), "nomodel".into()));
    let opencode = AgentPath::OpenCode {
        program: stand_in(),
        version: None,
    };
    let signals = heard_until(Launch::new(opencode, &*scratch, env, user()), |s| {
        matches!(s, Signal::OpenCodeFailed(_))
    });
    typed_after_words(&signals, |s| {
        matches!(s, Signal::OpenCodeFailed(ide_opencode::SessionError::SignIn))
    });

    let env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("SESSION_STAND_IN".to_string(), "acp".to_string()),
    ];
    let acp = ide_acp::Launch::new(
        "acp-test-agent",
        "Test Agent",
        stand_in(),
        vec!["auth".into()],
        env,
        &*scratch,
    );
    let launch = Launch::new(AgentPath::Acp(Box::new(acp)), &*scratch, Vec::new(), user());
    let signals = heard_until(launch, |s| matches!(s, Signal::AcpFailed(_)));
    typed_after_words(&signals, |s| {
        matches!(s, Signal::AcpFailed(ide_acp::SessionError::SignIn))
    });
}

#[test]
fn the_own_agent_continues_a_conversation_its_caller_read_from_the_store() {
    use ide_agent::stub::{Answer, Stub};
    let scratch = folder("own-stored");
    let project = scratch.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let store = scratch.join("thread.json");
    let answers = vec![
        Answer::events(&own_answer("First answer.")),
        Answer::events(&own_answer("Second answer.")),
    ];
    let stub = Stub::script(answers).unwrap();
    let mut harness = Harness::new();
    let config = own_config(&project, &stub.url(), Some(store.clone()));
    let launch = Launch::new(AgentPath::Own(Box::new(config)), &project, Vec::new(), user());
    let (first, watcher) = start(&mut harness, launch);
    let turn = send(&mut harness, first, "Build it");
    assert_eq!(harness.end_of(watcher, turn).outcome, Outcome::Succeeded);
    harness.registry.remove(first);
    let stored = ide_agent::store::load(&store).unwrap();
    let config = own_config(&project, &stub.url(), Some(store));
    let path = AgentPath::OwnStored(Box::new(config), Box::new(stored));
    let (second, watcher) = start(&mut harness, Launch::new(path, &project, Vec::new(), user()));
    harness.until_state("the stored conversation shown again", |r| {
        says(r, second, "First answer.")
    });
    let turn = send(&mut harness, second, "Again");
    assert_eq!(harness.end_of(watcher, turn).outcome, Outcome::Succeeded);
    harness.until_state("the second answer shown", |r| says(r, second, "Second answer."));
    let asked = stub.seen().last().map(|seen| seen.body.clone()).unwrap_or_default();
    assert!(
        asked.contains("Build it") && asked.contains("First answer."),
        "the conversation goes on: {asked}"
    );
}
