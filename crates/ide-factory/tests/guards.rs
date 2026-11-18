//! The guards that keep an order from passing G2, its gate set or the take-over queue by a side way (E164, E190,
//! E174): a partial acceptance closes only for the state it was given for, setup and network wait for the confirmed
//! gate set, an order made without a window waits for one, a late result moves nothing the user moved, and the queue
//! goes on whichever way its head leaves.

mod support;

use ide_factory::effect::Effect;
use ide_factory::event::{OpResult, Phase};
use ide_factory::{Condition, InputKind, PauseReason, Stage, Station, TakeOverBlocker, WaitReason};
use support::{App, WORKFLOW, rules};

fn app() -> App {
    App::new(Stage::One, rules(WORKFLOW))
}

/// An order whose G2 rejected a section, held in the review of what is left at its gate.
fn partially_accepted_in_review(app: &mut App) -> ide_factory::OrderId {
    let id = app.order("o1");
    app.settle();
    let fingerprint = app.fingerprint(&id);
    app.hold.insert("run_gate");
    app.send(InputKind::Accept {
        order: id.clone(),
        fingerprint,
        rejected: vec![ide_factory::HunkId("src/lib.rs#2".into())],
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Review);
    assert!(app.get(&id).partial());
    app.pending.clear();
    app.hold.clear();
    id
}

/// After new work of the builder the order waits for G2 again instead of being taken over (E164).
fn asks_g2_again(app: &App, id: &ide_factory::OrderId) {
    assert_eq!(app.condition(id), Condition::Waiting(WaitReason::Acceptance));
    assert_eq!(app.get(id).accepted(), None);
    assert!(!app.get(id).partial());
    assert!(!app.effects.iter().any(|e| matches!(e, Effect::TakeOver { .. })));
}

#[test]
fn back_to_the_builder_voids_a_partial_acceptance() {
    let mut app = app();
    let id = partially_accepted_in_review(&mut app);
    app.send(InputKind::BackToBuilder(id.clone(), "Also handle quotes.".into()));
    app.settle();
    asks_g2_again(&app, &id);
}

#[test]
fn changed_criteria_void_a_partial_acceptance() {
    let mut app = app();
    let id = partially_accepted_in_review(&mut app);
    app.send(InputKind::ChangeCriteria(
        id.clone(),
        "exports CSV\nquotes fields".into(),
    ));
    app.settle();
    asks_g2_again(&app, &id);
}

#[test]
fn work_by_hand_voids_a_partial_acceptance() {
    let mut app = app();
    let id = partially_accepted_in_review(&mut app);
    app.send(InputKind::ByHand(id.clone()));
    app.send(InputKind::BackToLeitstand(id.clone()));
    app.settle();
    asks_g2_again(&app, &id);
}

#[test]
fn a_rule_change_while_hunks_are_reversed_voids_the_partial_acceptance() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let fingerprint = app.fingerprint(&id);
    app.hold.insert("reject_hunks");
    app.send(InputKind::Accept {
        order: id.clone(),
        fingerprint,
        rejected: vec![ide_factory::HunkId("src/lib.rs#2".into())],
    });
    app.settle();
    let stricter = rules(&format!("{WORKFLOW}[limits]\nturns_per_station = 10\n"));
    app.send(InputKind::Rules(Box::new(stricter)));
    app.hold.clear();
    app.settle();
    asks_g2_again(&app, &id);
}

#[test]
fn a_rule_change_voids_an_acceptance_kept_over_a_moved_base() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.merges.push_back(OpResult::Merged {
        head: "m1".into(),
        tree: "tm1".into(),
        base: "base1".into(),
        moved: true,
    });
    app.identical = true;
    app.hold.insert("take_over");
    app.accept(&id);
    app.settle();
    assert!(app.get(&id).kept() && matches!(app.get(&id).phase(), Phase::TakingOver { .. }));
    app.pending.clear();
    app.hold.clear();
    let stricter = rules(&format!("{WORKFLOW}[limits]\nturns_per_station = 10\n"));
    app.send(InputKind::Rules(Box::new(stricter)));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    assert!(!app.get(&id).kept());
    assert_eq!(app.get(&id).accepted(), None);
}

#[test]
fn setup_and_the_agents_network_wait_for_the_confirmed_gate_set() {
    for (text, held) in [
        ("[worktree]\nsetup = \"make setup\"\n", "prepare_worktree"),
        ("[gates]\nnetwork = [\"crates.io\"]\n", "start_session"),
    ] {
        let text = format!("{text}{WORKFLOW}");
        let workflow = ide_factory::workflow::parse(&text).unwrap().workflow;
        let unconfirmed = ide_factory::workflow::rules::Rules::new(
            &ide_factory::workflow::UserDefaults::default(),
            Some(&workflow),
            &ide_factory::workflow::rules::Confirmations::default(),
        );
        let hash = unconfirmed.gate_hash().to_string();
        let mut app = App::new(Stage::One, unconfirmed);
        let id = app.order("o1");
        app.settle();
        assert_eq!(
            app.condition(&id),
            Condition::Waiting(WaitReason::GateCommandsChanged),
            "{held}"
        );
        assert_eq!(app.asked(held), 0, "{held}");
        app.send(InputKind::ConfirmGates(hash));
        app.settle();
        assert_eq!(app.station(&id), Station::Acceptance, "{held}");
        assert!(app.effects.iter().any(|e| match e {
            Effect::PrepareWorktree { setup, .. } => setup.is_some(),
            Effect::StartSession { network, .. } => network == &["crates.io"],
            _ => false,
        }));
    }
}

#[test]
fn an_order_made_without_a_window_starts_only_once_one_opens() {
    let mut app = app();
    app.send(InputKind::Windows(false));
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::NoWindow));
    assert_eq!(app.asked("prepare_worktree") + app.asked("start_session"), 0);
    app.send(InputKind::Windows(true));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

/// Two orders accepted into `main`, the first's take-over held and the second queued behind it.
fn queued_behind_the_first(app: &mut App) -> (ide_factory::OrderId, ide_factory::OrderId) {
    let first = app.order("o1");
    app.settle();
    let second = app.order("o2");
    app.settle();
    app.accept(&first);
    app.settle();
    app.accept(&second);
    app.settle();
    assert_eq!(*app.get(&second).phase(), Phase::Queue);
    (first, second)
}

#[test]
fn an_aborted_head_lets_the_take_over_queue_go_on() {
    let mut app = app();
    app.hold.insert("take_over");
    let (first, second) = queued_behind_the_first(&mut app);
    app.take("take_over").expect("the first's take-over");
    app.send(InputKind::Abort(first.clone()));
    app.hold.clear();
    app.settle();
    assert_eq!(app.condition(&first), Condition::Aborted);
    assert_eq!(app.station(&second), Station::Done);
}

#[test]
fn a_head_sent_back_to_the_review_lets_the_take_over_queue_go_on() {
    let mut app = app();
    app.merges.push_back(OpResult::Merged {
        head: "m1".into(),
        tree: "tm1".into(),
        base: "base1".into(),
        moved: true,
    });
    app.identical = false;
    app.hold.insert("compare_diffs");
    let (first, second) = queued_behind_the_first(&mut app);
    app.hold.clear();
    app.settle();
    assert_eq!(app.condition(&first), Condition::Waiting(WaitReason::Acceptance));
    assert_eq!(app.station(&second), Station::Done);
}

#[test]
fn a_blocked_head_lets_the_take_over_queue_go_on() {
    let mut app = app();
    app.takeovers.push_back(OpResult::Blocked {
        blocker: TakeOverBlocker::OtherBranch("feature".into()),
    });
    app.hold.insert("take_over");
    let (first, second) = queued_behind_the_first(&mut app);
    app.hold.clear();
    app.settle();
    assert!(matches!(
        app.condition(&first),
        Condition::Waiting(WaitReason::TakeOverBlocked { .. })
    ));
    assert_eq!(app.station(&second), Station::Done);
    app.send(InputKind::Continue(first.clone()));
    app.settle();
    assert_eq!(app.station(&first), Station::Done);
}

#[test]
fn a_late_commit_does_not_move_an_order_the_user_sent_back_to_the_builder() {
    let mut app = app();
    app.hold.insert("commit");
    let id = app.order("o1");
    app.settle();
    let commit = app.take("commit").expect("the commit after the turn");
    app.hold.insert("send_prompt");
    app.send(InputKind::BackToBuilder(id.clone(), "Rename the function.".into()));
    app.settle();
    assert!(matches!(app.get(&id).phase(), Phase::Turn { .. }));
    let reply = app.reply(&commit).expect("the commit's result");
    app.send(reply);
    app.settle();
    assert_eq!(app.station(&id), Station::Build);
    assert!(
        matches!(app.get(&id).phase(), Phase::Turn { .. }),
        "the builder's new turn goes on"
    );
    assert_eq!(
        app.get(&id).head().map(|(c, _)| c),
        Some("c1"),
        "the commit is recorded"
    );
    assert_eq!(app.asked("diff_names") + app.asked("run_gate"), 0);
}

#[test]
fn a_gate_set_changed_during_the_review_reruns_what_changed_and_the_evidence_binds_only_that() {
    let gates = "[[gates.command]]\nname = \"check\"\nrun = \"make check\"\n\
                 [[gates.command]]\nname = \"lint\"\nrun = \"make lint\"\n";
    let mut app = App::new(Stage::One, rules(gates));
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    let check = app.take("run_gate").expect("the first gate");
    let reply = app.reply(&check).unwrap();
    app.send(reply);
    let lint = app.take("run_gate").expect("the second gate");
    app.send(InputKind::Rules(Box::new(rules(
        &gates.replace("make check", "make test"),
    ))));
    let reply = app.reply(&lint).unwrap();
    app.send(reply);
    app.hold.clear();
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    let runs: Vec<String> = app
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::RunGate { run, .. } => Some(run.command.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(runs, ["make check", "make lint", "make test"]);
    let bound: Vec<Option<&str>> = app
        .get(&id)
        .evidence()
        .expect("sealed")
        .gates()
        .iter()
        .filter(|g| g.command.is_some())
        .map(|g| g.run.as_deref())
        .collect();
    assert_eq!(bound, [Some("make lint"), Some("make test")]);
}
