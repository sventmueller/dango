//! Results that arrive while an order does not run (E166, E167): each is recorded, nothing new starts, and resuming
//! goes on from what was recorded, a failed gate to the builder and a passed one not again.

mod support;

use ide_factory::effect::Effect;
use ide_factory::event::{Phase, TurnOutcome};
use ide_factory::gates::CommandOutcome;
use ide_factory::{Condition, InputKind, PauseReason, SessionEvent, Stage, Station};
use support::{App, WORKFLOW, rules};

fn app() -> App {
    App::new(Stage::One, rules(WORKFLOW))
}

/// An app whose order's gate runs, unanswered, while the Mac sleeps.
fn gate_while_asleep() -> (App, ide_factory::OrderId, Effect) {
    let mut app = app();
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    let gate = app.take("run_gate").expect("the gate runs");
    app.send(InputKind::Sleep);
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::Sleep));
    (app, id, gate)
}

/// The effects asked for since `before`, by their tags.
fn asked_since(app: &App, before: usize) -> Vec<String> {
    app.effects[before..]
        .iter()
        .map(support::kind)
        .filter(|k| k != "wake_lock")
        .collect()
}

#[test]
fn a_gate_that_passes_while_paused_starts_nothing_and_does_not_run_again() {
    let (mut app, id, gate) = gate_while_asleep();
    let before = app.effects.len();
    let result = app.reply(&gate).expect("a gate's result");
    app.send(result);
    assert_eq!(asked_since(&app, before), Vec::<String>::new());
    assert_eq!(
        app.get(&id).gates().iter().filter(|g| g.command.is_some()).count(),
        1,
        "the result is recorded"
    );
    assert_eq!(*app.get(&id).phase(), Phase::Idle);
    app.hold.clear();
    app.send(InputKind::Wake);
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.asked("run_gate"), 1, "the passed gate does not run again");
    app.assert_replays();
}

#[test]
fn a_gate_that_fails_while_paused_goes_back_to_the_builder_once_resumed() {
    let (mut app, id, gate) = gate_while_asleep();
    let Effect::RunGate {
        order, run_id, index, ..
    } = gate
    else {
        panic!("a gate")
    };
    let before = app.effects.len();
    app.send(InputKind::GateDone {
        order,
        run_id,
        index,
        outcome: CommandOutcome {
            exit: Some(2),
            excerpt: "error".into(),
            ..CommandOutcome::default()
        },
    });
    assert_eq!(
        asked_since(&app, before),
        Vec::<String>::new(),
        "no rework prompt while paused"
    );
    assert_eq!(app.station(&id), Station::Build);
    assert!(app.get(&id).findings().is_some());
    let prompts = app.asked("send_prompt");
    app.hold.clear();
    app.send(InputKind::Wake);
    app.settle();
    assert_eq!(
        app.asked("send_prompt"),
        prompts + 1,
        "the rework prompt goes out after waking"
    );
    assert_eq!(app.get(&id).review_rounds(), 1);
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.asked("run_gate"), 2);
    app.assert_replays();
}

#[test]
fn a_commit_that_ends_while_paused_is_recorded_and_its_ref_watch_follows_the_resume() {
    let mut app = app();
    app.hold.insert("commit");
    let id = app.order("o1");
    app.settle();
    let commit = app.take("commit").expect("the commit is asked for");
    app.send(InputKind::Windows(false));
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::NoWindow));
    let before = app.effects.len();
    let done = app.reply(&commit).expect("the commit's result");
    app.send(done);
    assert_eq!(asked_since(&app, before), Vec::<String>::new());
    assert_eq!(app.get(&id).open_operations(), 0, "the commit is recorded");
    assert!(app.get(&id).head().is_some_and(|(c, _)| c != "base0"));
    assert_eq!(*app.get(&id).phase(), Phase::Refs);
    app.hold.clear();
    app.send(InputKind::Windows(true));
    app.settle();
    assert_eq!(app.asked("commit"), 1);
    assert_eq!(app.station(&id), Station::Acceptance);
    app.assert_replays();
}

#[test]
fn a_turn_that_ends_while_paused_is_scanned_and_committed_after_waking_without_a_new_prompt() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    let prompt = app.get(&id).turn().unwrap().prompt();
    app.send(InputKind::Sleep);
    let before = app.effects.len();
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::TurnEnded {
            prompt: Some(prompt),
            outcome: TurnOutcome::Succeeded,
            answer: None,
        },
    ));
    assert_eq!(asked_since(&app, before), Vec::<String>::new());
    assert!(app.get(&id).turn().is_none(), "the turn's end is recorded");
    assert_eq!(*app.get(&id).phase(), Phase::TurnScan);
    app.hold.clear();
    app.pending.retain(|e| support::kind(e) != "send_prompt");
    app.send(InputKind::Wake);
    app.settle();
    assert_eq!(app.asked("send_prompt"), 1, "the ended turn is not prompted again");
    assert_eq!(app.asked("commit"), 1);
    assert_eq!(app.station(&id), Station::Acceptance);
    app.assert_replays();
}
