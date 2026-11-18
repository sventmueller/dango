//! The factory's central promise (E167): an order is nothing but its log applied in turn, so after every step the
//! running order and a fresh replay of the very log it wrote agree; when a write fails, the order goes back to what
//! its log says, and what earlier steps began goes on. Usage reports count against what the log kept, so a resumed
//! thread is not counted twice.

mod support;

use ide_factory::budget::UsageReport;
use ide_factory::effect::Effect;
use ide_factory::event::{OpResult, Phase};
use ide_factory::input::RequestKind;
use ide_factory::{
    Condition, InputKind, OrderId, PauseReason, RequestId, Role, SessionEvent, SessionKey, Stage, Station,
};
use support::{App, WORKFLOW, comparable, rules};

fn app(stage: Stage) -> App {
    App::new(stage, rules(WORKFLOW))
}

/// Settles and checks the promise.
fn settle(app: &mut App) {
    app.settle();
    app.assert_replays();
}

fn every_station() -> App {
    let mut app = app(Stage::One);
    let id = app.order("o1");
    settle(&mut app);
    app.accept(&id);
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Done);
    app
}

fn a_failing_gate_and_its_rework() -> App {
    let mut app = app(Stage::One);
    app.gates.push_back(ide_factory::gates::CommandOutcome {
        exit: Some(2),
        excerpt: "error".into(),
        ..ide_factory::gates::CommandOutcome::default()
    });
    let id = app.order("o1");
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Acceptance);
    app
}

fn a_partial_acceptance() -> App {
    let mut app = app(Stage::One);
    let id = app.order("o1");
    settle(&mut app);
    let fingerprint = app.fingerprint(&id);
    app.send(InputKind::Accept {
        order: id.clone(),
        fingerprint,
        rejected: vec![ide_factory::HunkId("src/lib.rs#2".into())],
    });
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Done);
    app
}

fn a_moved_base() -> App {
    let mut app = app(Stage::One);
    let id = app.order("o1");
    settle(&mut app);
    app.merges.push_back(OpResult::Merged {
        head: "m1".into(),
        tree: "tm1".into(),
        base: "base1".into(),
        moved: true,
    });
    app.identical = false;
    app.accept(&id);
    settle(&mut app);
    app.accept(&id);
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Done);
    app
}

fn a_conflict_and_a_queue() -> App {
    let mut app = app(Stage::One);
    let first = app.order("o1");
    settle(&mut app);
    let second = app.order("o2");
    settle(&mut app);
    app.merges.push_back(OpResult::Conflict {
        paths: vec!["src/lib.rs".into()],
    });
    app.accept(&first);
    settle(&mut app);
    app.hold.insert("take_over");
    app.accept(&first);
    settle(&mut app);
    app.accept(&second);
    settle(&mut app);
    app.hold.clear();
    settle(&mut app);
    assert_eq!(app.station(&first), Station::Done);
    assert_eq!(app.station(&second), Station::Done);
    app
}

fn a_question_and_an_abort() -> App {
    let mut app = app(Stage::One);
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    settle(&mut app);
    let session = app.turn_session(&id);
    let request = RequestId("q1".into());
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::Request {
            request: request.clone(),
            kind: RequestKind::Question,
        },
    ));
    app.assert_replays();
    app.send(InputKind::Answer {
        order: id.clone(),
        request,
        text: "Use commas.".into(),
    });
    app.assert_replays();
    app.send(InputKind::Abort(id.clone()));
    settle(&mut app);
    assert_eq!(app.condition(&id), Condition::Aborted);
    app
}

fn work_by_hand() -> App {
    let mut app = app(Stage::One);
    let id = app.order("o1");
    settle(&mut app);
    app.send(InputKind::BackToBuilder(id.clone(), "Rename the column.".into()));
    app.hold.insert("send_prompt");
    settle(&mut app);
    app.send(InputKind::ByHand(id.clone()));
    app.hold.clear();
    app.send(InputKind::BackToLeitstand(id.clone()));
    app.pending.retain(|e| support::kind(e) != "send_prompt");
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Acceptance);
    app
}

fn a_planner_and_a_reviewer() -> App {
    let mut app = app(Stage::Two);
    app.verdicts
        .push_back(r#"{"criteria":[{"id":"K1","verdict":"not_met","reason":"no header"}],"findings":[]}"#.into());
    app.usage = Some(UsageReport {
        dollars: Some(0.5),
        tokens: Some(100),
        tokens_reliable: true,
    });
    let id = app.order("o1");
    settle(&mut app);
    app.send(InputKind::Approve(id.clone()));
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(app.get(&id).spend().dollars() > 0.0);
    app
}

fn a_restart_in_the_middle() -> App {
    let mut app = app(Stage::One);
    app.hold.insert("run_gate");
    let id = app.order("o1");
    settle(&mut app);
    app.send(InputKind::Quit);
    let mut next = app.restart_with(rules(WORKFLOW));
    next.assert_replays();
    next.send(InputKind::Resume(None));
    settle(&mut next);
    assert_eq!(next.station(&id), Station::Acceptance);
    next
}

/// A scenario of the machine's tests, checked after every settled step.
type Scenario = fn() -> App;

#[test]
fn every_scenario_restores_to_what_ran() {
    let scenarios: [(&str, Scenario); 9] = [
        ("every station", every_station),
        ("a failing gate", a_failing_gate_and_its_rework),
        ("a partial acceptance", a_partial_acceptance),
        ("a moved base", a_moved_base),
        ("a conflict and a queue", a_conflict_and_a_queue),
        ("a question and an abort", a_question_and_an_abort),
        ("work by hand", work_by_hand),
        ("a planner and a reviewer", a_planner_and_a_reviewer),
        ("a restart in the middle", a_restart_in_the_middle),
    ];
    for (name, scenario) in scenarios {
        let app = scenario();
        assert!(app.factory.orders().count() > 0, "{name}");
        app.assert_replays();
    }
}

/// Order `id` as it runs and as its log says, apart from the pause no log entry holds.
fn assert_replays_but_the_pause(app: &App, id: &OrderId) {
    let without = |order: &ide_factory::Order| {
        let mut value = comparable(order);
        value.as_object_mut().expect("an object").remove("condition");
        value
    };
    assert_eq!(without(app.get(id)), without(&app.replayed(id)));
}

fn usage(id: &OrderId, session: &SessionKey, dollars: f64) -> InputKind {
    InputKind::Session(
        id.clone(),
        session.clone(),
        SessionEvent::Usage {
            report: UsageReport {
                dollars: Some(dollars),
                tokens: None,
                tokens_reliable: true,
            },
        },
    )
}

#[test]
fn spending_whose_entry_could_not_be_written_is_counted_with_the_next_report() {
    let mut app = app(Stage::One);
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    app.unwritable.insert(id.clone());
    app.send(usage(&id, &session, 2.0));
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::LogUnwritable));
    assert_eq!(
        app.get(&id).spend().dollars(),
        0.0,
        "what the log does not hold is not spent"
    );
    assert!(!app.effects.iter().any(|e| matches!(e, Effect::AddToDay { .. })));
    assert_replays_but_the_pause(&app, &id);
    app.unwritable.clear();
    app.send(usage(&id, &session, 3.0));
    assert_eq!(app.get(&id).spend().dollars(), 3.0, "the session's whole usage, once");
    assert_replays_but_the_pause(&app, &id);
    app.send(InputKind::Resume(Some(id.clone())));
    app.hold.clear();
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.get(&id).spend().dollars(), 3.0);
}

#[test]
fn a_failed_write_leaves_what_earlier_steps_began_running() {
    let mut app = app(Stage::One);
    app.hold.insert("commit");
    let id = app.order("o1");
    app.settle();
    assert!(matches!(app.get(&id).phase(), Phase::Committing { .. }));
    let builder = app.get(&id).session(Role::Builder).unwrap().session().clone();
    app.unwritable.insert(id.clone());
    app.send(InputKind::Session(id.clone(), builder, SessionEvent::Intervened));
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::LogUnwritable));
    assert_eq!(app.get(&id).open_operations(), 1, "the commit begun before still runs");
    assert_eq!(app.get(&id).interventions(), 0);
    assert_replays_but_the_pause(&app, &id);
    app.unwritable.clear();
    app.hold.clear();
    app.settle();
    assert_eq!(app.get(&id).open_operations(), 0, "its result is taken");
    app.send(InputKind::Resume(Some(id.clone())));
    settle(&mut app);
    assert_eq!(app.asked("commit"), 1, "the commit is made once");
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn an_order_whose_creation_could_not_be_written_is_gone() {
    let mut app = app(Stage::One);
    let id = OrderId("o1".into());
    app.unwritable.insert(id.clone());
    app.order("o1");
    assert!(app.factory.order(&id).is_none());
    assert!(!app.logs.contains_key(&id));
    assert_eq!(app.asked("prepare_worktree"), 0);
    app.unwritable.clear();
    app.order("o1");
    settle(&mut app);
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_resumed_thread_that_reports_its_whole_usage_is_not_counted_twice() {
    let mut app = app(Stage::One);
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    app.send(usage(&id, &session, 2.0));
    app.send(InputKind::Quit);
    let mut next = app.restart_with(rules(WORKFLOW));
    next.hold.insert("send_prompt");
    next.send(InputKind::Resume(Some(id.clone())));
    next.settle();
    assert!(
        next.effects
            .iter()
            .any(|e| matches!(e, Effect::StartSession { resume: Some(_), .. })),
        "the builder's thread goes on"
    );
    let resumed = next.turn_session(&id);
    next.send(usage(&id, &resumed, 2.5));
    assert_eq!(next.get(&id).spend().dollars(), 2.5);
    next.assert_replays();
}

#[test]
fn a_resumed_thread_that_counts_from_its_new_process_is_counted_whole() {
    let mut app = app(Stage::One);
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    app.send(usage(&id, &session, 2.0));
    app.send(InputKind::Quit);
    let mut next = app.restart_with(rules(WORKFLOW));
    next.hold.insert("send_prompt");
    next.send(InputKind::Resume(Some(id.clone())));
    next.settle();
    let resumed = next.turn_session(&id);
    next.send(usage(&id, &resumed, 0.4));
    assert!((next.get(&id).spend().dollars() - 2.4).abs() < 1e-9);
    next.send(usage(&id, &resumed, 0.6));
    assert!((next.get(&id).spend().dollars() - 2.6).abs() < 1e-9);
    next.assert_replays();
}
