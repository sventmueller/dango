//! What a scripted session's driver reports, mapped into the factory's terms: the start once the session is ready,
//! the typed end of each turn with its prompt and answer, a question left to the user, and the end of the process.

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_session::{End, Script, ScriptTurn, Step};

use super::*;

/// The longest a test waits.
const WAIT: Duration = Duration::from_secs(20);

struct Harness {
    registry: Registry,
    woken: Receiver<SessionId>,
    session: SessionId,
    link: Link,
    mapped: Vec<InputKind>,
    _folder: ide_core::testing::Scratch,
}

impl Harness {
    fn new(turns: Vec<ScriptTurn>) -> Harness {
        let folder = ide_core::testing::scratch("leitstand-sessions");
        let (tx, woken) = channel();
        let tx = Mutex::new(tx);
        let wake: Arc<dyn ide_session::Wake> = Arc::new(move |id: SessionId| {
            let _ = tx.lock().map(|tx| tx.send(id));
        });
        let mut registry = Registry::new(Supervisor::new(), wake);
        let contract = Contract::Order(ide_session::Order {
            order: "o-1".into(),
            role: ide_session::Role::Builder,
            worktree: folder.to_path_buf(),
            build_dir: None,
            read_roots: Vec::new(),
            allowed_domains: Vec::new(),
            refuse_outside: false,
        });
        let script = Script {
            turns,
            pace: Duration::ZERO,
        };
        let rule = ide_session::rule::for_contract(&contract);
        let launch = Launch::new(AgentPath::Scripted(script), &*folder, Vec::new(), contract);
        let session = registry.start(launch, rule).unwrap();
        let subscriber = registry.get_mut(session).unwrap().subscribe();
        let link = Link {
            root: folder.to_path_buf(),
            order: OrderId("o-1".into()),
            title: "CSV".into(),
            key: SessionKey("k-1".into()),
            role: ide_factory::Role::Builder,
            agent: Agent::Claude,
            subscriber,
            prompts: BTreeMap::new(),
            requests: BTreeMap::new(),
            started: false,
            resume: None,
            activity: None,
            usage: None,
            limits: Vec::new(),
        };
        Harness {
            registry,
            woken,
            session,
            link,
            mapped: Vec::new(),
            _folder: folder,
        }
    }

    /// Plays the main thread until `done` holds for what was mapped.
    fn until(&mut self, what: &str, done: impl Fn(&[InputKind]) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.mapped) {
            assert!(
                Instant::now() < deadline,
                "waited in vain for {what}: {:?}",
                self.mapped
            );
            let Ok(id) = self.woken.recv_timeout(Duration::from_millis(50)) else {
                continue;
            };
            self.registry.flush(id);
            let driver = self.registry.get_mut(id).unwrap();
            let signals = driver.take(self.link.subscriber).map(|b| b.signals).unwrap_or_default();
            let view = View {
                thread: driver.thread(),
                queued: driver.queued(),
                session_id: driver.session_id(),
                now: Stamp::new(1_000, 1_700_000_000_000),
            };
            let mapped = map(&mut self.link, &signals, &view);
            self.mapped.extend(mapped.inputs);
        }
    }

    fn send(&mut self, text: &str, prompt: u64) {
        let driver = self.registry.get_mut(self.session).unwrap();
        let turn = driver.send(Prompt::new(text)).unwrap();
        self.link.prompts.insert(turn, PromptId(prompt));
    }
}

fn session_event(input: &InputKind) -> Option<&SessionEvent> {
    match input {
        InputKind::Session(order, key, event) if order.0 == "o-1" && key.0 == "k-1" => Some(event),
        _ => None,
    }
}

#[test]
fn a_session_starts_its_turn_ends_with_its_prompt_and_answer_and_a_question_waits_for_the_user() {
    let mut harness = Harness::new(vec![
        ScriptTurn::new(vec![Step::Say("Exported.".into())], End::Succeed),
        ScriptTurn::new(vec![Step::Question("Which separator?".into())], End::Succeed),
    ]);
    harness.until("the start", |mapped| {
        mapped
            .iter()
            .any(|i| matches!(i, InputKind::SessionStarted { session, .. } if session.0 == "k-1"))
    });
    harness.send("Build it", 1);
    harness.until("the end of the turn", |mapped| {
        mapped
            .iter()
            .filter_map(session_event)
            .any(|e| matches!(e, SessionEvent::TurnEnded { .. }))
    });
    let ended = harness.mapped.iter().filter_map(session_event).find_map(|e| match e {
        SessionEvent::TurnEnded {
            prompt,
            outcome,
            answer,
        } => Some((*prompt, outcome.clone(), answer.clone())),
        _ => None,
    });
    assert_eq!(
        ended,
        Some((Some(PromptId(1)), TurnOutcome::Succeeded, Some("Exported.".into())))
    );
    assert!(
        harness
            .mapped
            .iter()
            .filter_map(session_event)
            .any(|e| matches!(e, SessionEvent::Activity { .. })),
        "the activity reaches the stall rule: {:?}",
        harness.mapped
    );
    harness.send("Go on", 2);
    harness.until("the question", |mapped| {
        mapped.iter().filter_map(session_event).any(|e| {
            matches!(
                e,
                SessionEvent::Request {
                    kind: RequestKind::Question,
                    ..
                }
            )
        })
    });
}

#[test]
fn a_crash_after_the_start_is_the_end_of_the_session() {
    let mut harness = Harness::new(vec![ScriptTurn::new(Vec::new(), End::Crash("gone".into()))]);
    harness.until("the start", |mapped| {
        mapped.iter().any(|i| matches!(i, InputKind::SessionStarted { .. }))
    });
    harness.send("Build it", 1);
    harness.until("the end", |mapped| {
        mapped
            .iter()
            .filter_map(session_event)
            .any(|e| matches!(e, SessionEvent::Exited))
    });
    let outcome = harness.mapped.iter().filter_map(session_event).find_map(|e| match e {
        SessionEvent::TurnEnded { outcome, .. } => Some(outcome.clone()),
        _ => None,
    });
    assert!(matches!(
        outcome,
        Some(TurnOutcome::ConnectionLost | TurnOutcome::Failed(_))
    ));
}

#[test]
fn a_launch_error_is_a_readiness_routing_records() {
    assert_eq!(
        readiness_of(&LaunchError::NotFound("/x".into())),
        Readiness::NotInstalled
    );
    assert!(matches!(
        readiness_of(&LaunchError::NoRole(ide_session::PathKind::OpenCode)),
        Readiness::StartFailed(_)
    ));
    assert_eq!(window_seconds("five_hour"), 5 * 60 * 60);
    assert_eq!(window_seconds("seven_day_opus"), 7 * 24 * 60 * 60);
    assert_eq!(service(&Agent::Claude, true), "claude-subscription");
}
