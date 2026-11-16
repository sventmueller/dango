//! Restarts (E167, C66): orders come back from their logs paused, an operation begun without its end is verified
//! before anything repeats, a failed log write starts no side effect, and a log cut at any byte restores to the
//! state of its complete lines (E96: fixed seed and case count, like the other crates' properties).

mod support;

use ide_factory::event::{Event, OpResult, Phase};
use ide_factory::log::read;
use ide_factory::{Condition, Factory, InputKind, PauseReason, Stage, Stamp, Station};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use support::{App, WORKFLOW, rules};

fn restart(app: &App) -> App {
    restart_with(app, rules(WORKFLOW))
}

fn restart_with(app: &App, rules: ide_factory::workflow::rules::Rules) -> App {
    let logs: Vec<String> = app.logs.values().cloned().collect();
    let restored = Factory::restore(Stage::One, rules, &logs, Stamp::new(5, app.wall + 1000));
    assert!(restored.problems.is_empty(), "{:?}", restored.problems);
    let mut next = App::with(restored.factory);
    next.wall = app.wall + 2000;
    next.logs = app.logs.clone();
    for entry in &restored.step.log {
        next.logs
            .entry(entry.order.clone())
            .or_default()
            .push_str(&entry.line());
    }
    next.effects.extend(restored.step.effects.iter().cloned());
    next.pending.extend(restored.step.effects);
    next
}

#[test]
fn quitting_pauses_running_orders_and_a_restart_brings_them_back_paused() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Quit);
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::AppEnded));
    let mut next = restart(&app);
    assert_eq!(next.condition(&id), Condition::Paused(PauseReason::AppEnded));
    assert!(next.get(&id).turn().is_none());
    assert_eq!(next.asked("send_prompt"), 0, "nothing is sent after a restart (E64)");
    next.send(InputKind::Resume(None));
    next.settle();
    assert_eq!(next.station(&id), Station::Acceptance);
    let resumed = next
        .effects
        .iter()
        .any(|e| matches!(e, ide_factory::Effect::StartSession { resume: Some(token), .. } if token == "resume-1"));
    assert!(resumed, "the builder's session is resumed, not started anew");
}

#[test]
fn a_crash_without_quitting_also_comes_back_paused() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    let next = restart(&app);
    assert_eq!(next.condition(&id), Condition::Paused(PauseReason::AppEnded));
    let last = read(&next.logs[&id]).entries.last().cloned().unwrap();
    assert!(matches!(last.event, Event::Condition { .. }), "the pause is logged");
}

#[test]
fn orders_resume_by_themselves_only_when_the_setting_says_so() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let mut defaults = ide_factory::workflow::UserDefaults::default();
    defaults.set_resume_after_restart(true);
    let mut next = restart_with(&app, support::rules_with(&defaults, WORKFLOW));
    assert_eq!(next.condition(&id), Condition::Running);
    next.settle();
    assert_eq!(next.station(&id), Station::Acceptance);
}

/// An app whose order's commit after the first turn is begun but not answered.
fn committing() -> (App, ide_factory::OrderId) {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("commit");
    let id = app.order("o1");
    app.settle();
    assert!(matches!(app.get(&id).phase(), Phase::Committing { .. }));
    (app, id)
}

#[test]
fn a_commit_that_happened_before_its_end_was_logged_is_not_made_again() {
    let (app, id) = committing();
    let mut next = restart(&app);
    assert_eq!(next.asked("verify"), 1);
    next.verified.push_back(Some(OpResult::Committed {
        commit: "c1".into(),
        tree: "t1".into(),
    }));
    next.settle();
    assert_eq!(*next.get(&id).phase(), Phase::Refs);
    assert_eq!(next.get(&id).head(), Some(("c1", "t1")));
    next.send(InputKind::Resume(Some(id.clone())));
    next.settle();
    assert_eq!(next.asked("commit"), 0);
    assert_eq!(next.station(&id), Station::Acceptance);
}

#[test]
fn a_commit_that_did_not_happen_is_scanned_and_made_again() {
    let (app, id) = committing();
    let mut next = restart(&app);
    next.verified.push_back(None);
    next.settle();
    assert_eq!(*next.get(&id).phase(), Phase::TurnScan);
    next.send(InputKind::Resume(Some(id.clone())));
    next.settle();
    assert_eq!(next.asked("commit"), 1);
    assert_eq!(next.station(&id), Station::Acceptance);
}

#[test]
fn a_failed_log_write_starts_no_side_effect_and_pauses() {
    let (mut app, id) = committing();
    app.take("commit").unwrap();
    app.send(InputKind::LogFailed(id.clone()));
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::LogUnwritable));
    assert_eq!(app.get(&id).open_operations(), 0);
    app.hold.clear();
    app.send(InputKind::Resume(Some(id.clone())));
    app.settle();
    assert_eq!(app.asked("commit"), 2, "the commit is asked for again");
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_take_over_interrupted_before_its_end_was_logged_is_checked() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let id = app.order("o1");
    app.settle();
    app.hold.insert("take_over");
    app.accept(&id);
    app.settle();
    let mut next = restart(&app);
    next.verified
        .push_back(Some(OpResult::TakenOver { commit: "m".into() }));
    next.settle();
    assert!(next.get(&id).taken_over());
    next.send(InputKind::Resume(Some(id.clone())));
    next.settle();
    assert_eq!(next.station(&id), Station::Done);
    assert_eq!(next.asked("take_over"), 0);
}

/// The log of one order taken through every station.
fn full_log() -> String {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.gates.push_back(ide_factory::gates::CommandOutcome {
        exit: Some(2),
        excerpt: "error".into(),
        ..ide_factory::gates::CommandOutcome::default()
    });
    let id = app.order("o1");
    app.settle();
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    app.logs[&id].clone()
}

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x00fa_c702),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_log_cut_at_any_byte_restores_to_its_complete_lines(fraction in 0.0f64..=1.0) {
        let text = full_log();
        prop_assert!(text.is_ascii());
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss, reason = "a position within the text")]
        let cut = ((text.len() as f64) * fraction) as usize;
        let cut = cut.min(text.len());
        let prefix = &text[..cut];
        let lines_end = prefix.rfind('\n').map_or(0, |n| n + 1);
        let rest_is_an_entry = read(&prefix[lines_end..]).entries.len() == 1;
        let whole = if rest_is_an_entry { prefix } else { &text[..lines_end] };
        let at = Stamp::new(1, 1);
        let restored = Factory::restore(Stage::One, rules(WORKFLOW), &[prefix.to_string()], at);
        let oracle = Factory::restore(Stage::One, rules(WORKFLOW), &[whole.to_string()], at);
        let id = ide_factory::OrderId("o1".into());
        prop_assert_eq!(restored.factory.order(&id), oracle.factory.order(&id));
        let cut_tail = prefix.len() > whole.len();
        prop_assert_eq!(restored.problems.iter().any(|p| p.cut), cut_tail);
        if let Some(order) = restored.factory.order(&id) {
            prop_assert!(*order.condition() != Condition::Running || order.station() == Station::Done);
            prop_assert!(order.turn().is_none());
        }
    }
}
