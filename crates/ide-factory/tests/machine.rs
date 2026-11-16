//! The state machine of E166 driven through a scripted app: the stations an order passes, the conditions it waits
//! or pauses in, and that only the factory moves it on.

mod support;

use ide_factory::budget::{Budget, Unit, UsageReport};
use ide_factory::effect::Effect;
use ide_factory::event::{Event, OpResult, Phase, TurnOutcome};
use ide_factory::gates::{CommandOutcome, TestCount};
use ide_factory::input::{RefChange, RequestKind};
use ide_factory::log::read;
use ide_factory::options::{ChangeRefusal, OptionChange, RiskChoice};
use ide_factory::relevance::{Change, ChangeKind};
use ide_factory::{
    Condition, FailReason, InputKind, PauseReason, Refused, RequestId, RiskClass, SessionEvent, Stage, Station,
    TakeOverBlocker, WaitReason,
};
use support::{App, WORKFLOW, rules};

fn app() -> App {
    App::new(Stage::One, rules(WORKFLOW))
}

fn events(app: &App, id: &ide_factory::OrderId) -> Vec<Event> {
    read(&app.logs[id]).entries.into_iter().map(|e| e.event).collect()
}

#[test]
fn an_order_runs_through_every_station_to_done() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    let visited: Vec<Station> = app.get(&id).visits().iter().map(|v| v.station).collect();
    assert_eq!(
        visited,
        [Station::Intake, Station::Build, Station::Review, Station::Acceptance]
    );
    let evidence = app.get(&id).evidence().expect("sealed");
    assert_eq!(evidence.commit(), "c1");
    assert_eq!(evidence.bound().tree, "t1");
    assert!(
        evidence
            .gates()
            .iter()
            .any(|g| g.name == "check" && g.failure.is_none())
    );
    assert!(app.effects.iter().any(|e| matches!(e, Effect::Notify { .. })));
    assert!(!app.factory.holds_wake_lock(), "nothing runs while the order waits");
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert!(app.get(&id).finished());
    assert_eq!(app.get(&id).worktree(), None);
    let removals: Vec<String> = app
        .effects
        .iter()
        .filter(|e| matches!(e, Effect::RemoveCheckpoints { .. } | Effect::RemoveWorktree { .. }))
        .map(support::kind)
        .collect();
    assert_eq!(
        removals,
        ["remove_checkpoints", "remove_worktree"],
        "checkpoints go before the worktree (C70)"
    );
}

#[test]
fn stage_one_logs_the_specification_as_skipped_and_runs_no_reviewer() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let log = events(&app, &id);
    assert!(log.iter().any(|e| matches!(
        e,
        Event::Skipped {
            station: Station::Spec,
            why: ide_factory::event::SkipWhy::UserWroteSpec
        }
    )));
    assert!(log.iter().any(|e| matches!(
        e,
        Event::Skipped {
            why: ide_factory::event::SkipWhy::NoPathScope,
            ..
        }
    )));
    assert_eq!(app.asked("collect_review_input"), 0);
}

#[test]
fn every_side_effect_is_logged_as_begun_and_finished_with_its_operation() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.accept(&id);
    app.settle();
    let log = events(&app, &id);
    let begun: Vec<_> = log
        .iter()
        .filter_map(|e| match e {
            Event::Begun { op, target, .. } => Some((op.clone(), target.clone())),
            _ => None,
        })
        .collect();
    let finished: Vec<_> = log
        .iter()
        .filter_map(|e| match e {
            Event::Finished { op, .. } => Some(op.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        begun.len(),
        6,
        "worktree, commit, merge, take-over, checkpoints, worktree removal"
    );
    assert_eq!(begun.iter().map(|(op, _)| op.clone()).collect::<Vec<_>>(), finished);
    let commit = begun.iter().find_map(|(_, t)| match t {
        ide_factory::event::TargetState::CommitWithTrailer { operation, .. } => Some(operation.clone()),
        _ => None,
    });
    assert!(commit.is_some(), "a commit names its operation for the restart's check");
}

#[test]
fn a_second_order_waits_in_intake_while_one_runs() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let first = app.order("o1");
    app.settle();
    let second = app.order("o2");
    app.settle();
    assert_eq!(app.station(&second), Station::Intake);
    assert_eq!(*app.get(&second).phase(), Phase::Slot);
    app.hold.clear();
    let prompt = app.take("send_prompt").expect("the first order's turn");
    let reply = app.reply(&prompt).expect("a turn end");
    app.send(reply);
    app.settle();
    assert_eq!(app.condition(&first), Condition::Waiting(WaitReason::Acceptance));
    assert_eq!(app.condition(&second), Condition::Waiting(WaitReason::Acceptance));
}

#[test]
fn a_failing_gate_goes_back_to_the_builder_with_its_log_and_three_rounds_go_to_the_user() {
    let mut app = app();
    let failing = CommandOutcome {
        exit: Some(1),
        excerpt: "test parser ... FAILED".into(),
        tests: Some(TestCount {
            ran: 3,
            failed: 1,
            skipped: 0,
        }),
        ..CommandOutcome::default()
    };
    app.gates.extend([failing.clone(), failing.clone(), failing]);
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::ReviewRoundsSpent));
    assert_eq!(app.get(&id).review_rounds(), 3);
    let reworks: Vec<String> = app
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::SendPrompt { text, .. } if text.contains("did not pass") => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(reworks.len(), 2);
    assert!(
        reworks[0].contains("FAILED") && reworks[0].contains("untrusted"),
        "{}",
        reworks[0]
    );
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance, "the next round passes");
}

#[test]
fn a_gate_that_runs_no_test_fails() {
    let mut app = app();
    app.gates.push_back(CommandOutcome {
        exit: Some(0),
        tests: Some(TestCount::default()),
        ..CommandOutcome::default()
    });
    let id = app.order("o1");
    app.settle();
    let record = &app.get(&id).evidence().expect("sealed after the second round").gates();
    assert!(record.iter().all(|g| g.failure.is_none()));
    assert_eq!(app.get(&id).review_rounds(), 1);
}

#[test]
fn changed_agent_configuration_waits_for_the_user_before_the_review() {
    let mut app = app();
    app.diffs
        .push_back(vec![Change::new("CLAUDE.md", ChangeKind::Modified)]);
    let id = app.order("o1");
    app.settle();
    assert_eq!(
        app.condition(&id),
        Condition::Waiting(WaitReason::AgentConfigChanged {
            paths: vec!["CLAUDE.md".into()]
        })
    );
    assert_eq!(app.asked("run_gate"), 0);
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    let deviations = app.get(&id).evidence().expect("sealed").deviations().to_vec();
    assert!(matches!(
        deviations[0],
        ide_factory::evidence::Deviation::AgentConfigChanged { .. }
    ));
}

#[test]
fn a_measured_class_above_the_skipped_g1_waits_and_g2_at_high_needs_the_diff_opened() {
    let mut app = app();
    app.diffs
        .push_back(vec![Change::new("tests/parser.rs", ChangeKind::Modified)]);
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::LateSpecApproval));
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.get(&id).risk(), RiskClass::High);
    app.accept(&id);
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::OpenDiffFirst)
    ));
    app.send(InputKind::DiffOpened(id.clone()));
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
}

#[test]
fn an_unexplained_ref_change_waits_for_the_user() {
    let mut app = app();
    app.refs.push_back(vec![
        RefChange {
            name: "refs/heads/main".into(),
            before: Some("a".into()),
            after: Some("b".into()),
            by_dango: false,
        },
        RefChange {
            name: "refs/heads/order/o1".into(),
            before: None,
            after: Some("c1".into()),
            by_dango: false,
        },
    ]);
    let id = app.order("o1");
    app.settle();
    assert_eq!(
        app.condition(&id),
        Condition::Waiting(WaitReason::RefsChanged {
            refs: vec!["refs/heads/main".into()]
        })
    );
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn stale_evidence_is_not_accepted() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Accept {
        order: id.clone(),
        fingerprint: "old".into(),
        rejected: Vec::new(),
    });
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::StaleEvidence)
    ));
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn partial_acceptance_commits_the_new_state_reruns_the_gates_and_closes_g2_for_it() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let first = app.fingerprint(&id);
    app.send(InputKind::Accept {
        order: id.clone(),
        fingerprint: first.clone(),
        rejected: vec![ide_factory::HunkId("src/lib.rs#2".into())],
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(app.asked("reject_hunks"), 1);
    assert_eq!(app.asked("run_gate"), 2, "the gates ran for the new state");
    let accepted: Vec<String> = events(&app, &id)
        .into_iter()
        .filter_map(|e| match e {
            Event::Accepted { fingerprint, .. } => Some(fingerprint),
            _ => None,
        })
        .collect();
    assert_eq!(accepted.len(), 2);
    assert_ne!(accepted[0], accepted[1]);
}

#[test]
fn a_moved_base_with_the_same_diff_keeps_g2_but_reruns_the_gates() {
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
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(app.asked("run_gate"), 2);
    assert!(
        app.get(&id)
            .evidence()
            .expect("sealed")
            .deviations()
            .contains(&ide_factory::evidence::Deviation::BaseUpdated)
    );
}

#[test]
fn a_moved_base_with_another_diff_repeats_g2() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.merges.push_back(OpResult::Merged {
        head: "m1".into(),
        tree: "tm1".into(),
        base: "base1".into(),
        moved: true,
    });
    app.identical = false;
    app.accept(&id);
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    assert_eq!(app.get(&id).base().commit, "base1");
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
}

#[test]
fn a_conflict_with_the_target_goes_back_to_the_builder() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.merges.push_back(OpResult::Conflict {
        paths: vec!["src/lib.rs".into()],
    });
    app.accept(&id);
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("conflicts")))
    );
    assert_eq!(app.get(&id).accepted(), None, "G2 is asked again");
}

#[test]
fn a_take_over_onto_a_working_copy_elsewhere_waits_and_goes_on_when_told() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.takeovers.push_back(OpResult::Blocked {
        blocker: TakeOverBlocker::OtherBranch("feature".into()),
    });
    app.accept(&id);
    app.settle();
    assert_eq!(
        app.condition(&id),
        Condition::Waiting(WaitReason::TakeOverBlocked {
            blocker: TakeOverBlocker::OtherBranch("feature".into())
        })
    );
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
}

#[test]
fn take_overs_into_one_branch_queue_one_at_a_time() {
    let mut app = app();
    let first = app.order("o1");
    app.settle();
    let second = app.order("o2");
    app.settle();
    app.hold.insert("take_over");
    app.accept(&first);
    app.settle();
    app.accept(&second);
    app.settle();
    assert_eq!(*app.get(&second).phase(), Phase::Queue);
    app.hold.clear();
    app.settle();
    assert_eq!(app.station(&first), Station::Done);
    assert_eq!(app.station(&second), Station::Done);
}

#[test]
fn a_free_text_question_waits_and_its_answer_goes_on() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    let request = RequestId("q1".into());
    app.send(InputKind::Session(
        id.clone(),
        session.clone(),
        SessionEvent::Request {
            request: request.clone(),
            kind: RequestKind::Question,
        },
    ));
    assert_eq!(
        app.condition(&id),
        Condition::Waiting(WaitReason::Question {
            request: request.clone()
        })
    );
    app.send(InputKind::Answer {
        order: id.clone(),
        request,
        text: "Use commas.".into(),
    });
    assert_eq!(app.condition(&id), Condition::Running);
    assert!(matches!(app.get(&id).phase(), Phase::Turn { .. }));
    assert!(app.effects.iter().any(|e| matches!(e, Effect::AnswerRequest { .. })));
}

#[test]
fn a_plan_request_is_refused() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::Request {
            request: RequestId("p".into()),
            kind: RequestKind::Plan,
        },
    ));
    assert_eq!(app.condition(&id), Condition::Running);
    assert!(app.effects.iter().any(|e| matches!(
        e,
        Effect::AnswerRequest {
            answer: ide_factory::effect::Answer::Refuse(_),
            ..
        }
    )));
}

#[test]
fn a_turn_the_user_cancelled_waits_and_continue_resumes() {
    let mut app = app();
    app.turns.push_back(TurnOutcome::Cancelled);
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::CancelledTurn));
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_successful_turn_waits_for_its_subagents_and_stops_background_work_before_the_gate() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    let session = app.turn_session(&id);
    let prompt = app.get(&id).turn().expect("turn").prompt();
    let activity = |subagents, background| SessionEvent::Activity {
        running: true,
        tool_since: None,
        subagents,
        background,
        queued: 0,
    };
    app.send(InputKind::Session(id.clone(), session.clone(), activity(1, 1)));
    app.send(InputKind::Session(
        id.clone(),
        session.clone(),
        SessionEvent::TurnEnded {
            prompt: Some(prompt),
            outcome: TurnOutcome::Succeeded,
            answer: None,
        },
    ));
    assert!(
        matches!(app.get(&id).phase(), Phase::Turn { .. }),
        "a subagent still runs"
    );
    app.send(InputKind::Session(id.clone(), session, activity(0, 1)));
    assert_eq!(*app.get(&id).phase(), Phase::TurnScan);
    assert_eq!(app.asked("stop_background"), 1);
}

#[test]
fn abort_removes_checkpoints_then_the_worktree() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Abort(id.clone()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Aborted);
    assert_eq!(app.get(&id).worktree(), None);
    assert!(app.get(&id).checkpoints_removed());
    assert_eq!(app.asked("stop_turn"), 1);
}

#[test]
fn by_hand_and_back_to_the_leitstand_end_the_station_through_its_gate() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::BackToBuilder(id.clone(), "Rename the column.".into()));
    app.hold.insert("send_prompt");
    app.settle();
    app.send(InputKind::ByHand(id.clone()));
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::ByHand));
    app.hold.clear();
    app.send(InputKind::BackToLeitstand(id.clone()));
    app.pending.retain(|e| support::kind(e) != "send_prompt");
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(
        app.get(&id)
            .evidence()
            .expect("sealed")
            .deviations()
            .contains(&ide_factory::evidence::Deviation::ByHand)
    );
}

#[test]
fn changed_criteria_void_the_evidence_and_go_back_to_the_builder() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let first = app.fingerprint(&id);
    app.send(InputKind::ChangeCriteria(
        id.clone(),
        "- exports CSV\n- quotes fields\n".into(),
    ));
    app.settle();
    assert_eq!(app.get(&id).criteria().len(), 2);
    assert_eq!(app.get(&id).criteria()[0].id(), "K1");
    assert_ne!(app.fingerprint(&id), first);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
}

#[test]
fn options_lock_when_their_station_begins_and_a_budget_never_drops_below_the_spending() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::ChangeOption(
        id.clone(),
        OptionChange::Risk(RiskChoice::Fixed(RiskClass::High)),
    ));
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::Option { .. })
    ));
    let session = app.turn_session(&id);
    let report = UsageReport {
        dollars: Some(3.0),
        tokens: None,
        tokens_reliable: true,
    };
    app.send(InputKind::Session(id.clone(), session, SessionEvent::Usage { report }));
    app.send(InputKind::ChangeOption(
        id.clone(),
        OptionChange::Budget(Some(Budget::new(Some(2.0), None, None))),
    ));
    assert_eq!(
        app.refusals.last().map(|r| &r.why),
        Some(&Refused::Option {
            refusal: ChangeRefusal::BelowSpent { unit: Unit::Dollars }
        })
    );
    assert_eq!(app.get(&id).options().budget, None, "the refused budget is not taken");
    app.send(InputKind::ChangeOption(
        id.clone(),
        OptionChange::Budget(Some(Budget::new(Some(5.0), None, None))),
    ));
    assert_eq!(app.get(&id).options().budget, Some(Budget::new(Some(5.0), None, None)));
}

#[test]
fn a_gate_result_from_before_a_pause_is_not_taken_for_the_run_that_replaced_it() {
    let mut app = app();
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    let Some(Effect::RunGate { run_id: old, index, .. }) = app.take("run_gate") else {
        panic!("the gate runs")
    };
    app.send(InputKind::Quit);
    app.send(InputKind::Resume(Some(id.clone())));
    assert_eq!(
        app.asked("stop_gate"),
        1,
        "the gate still running from before is stopped"
    );
    let Some(Effect::RunGate { run_id: new, .. }) = app.take("run_gate") else {
        panic!("the gate runs again")
    };
    assert_ne!(old, new);
    app.send(InputKind::GateDone {
        order: id.clone(),
        run_id: old,
        index,
        outcome: CommandOutcome {
            exit: Some(2),
            ..CommandOutcome::default()
        },
    });
    assert_eq!(
        *app.get(&id).phase(),
        Phase::Gate { index },
        "the old run's result is not taken"
    );
    app.send(InputKind::GateDone {
        order: id.clone(),
        run_id: new,
        index,
        outcome: support::passing_gate(),
    });
    app.hold.clear();
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.get(&id).review_rounds(), 0);
    assert!(app.get(&id).findings().is_none());
}

#[test]
fn a_failed_side_effect_fails_the_station_and_continue_tries_again() {
    let mut app = app();
    app.hold.insert("prepare_worktree");
    let id = app.order("o1");
    app.settle();
    let Some(Effect::PrepareWorktree { op, .. }) = app.take("prepare_worktree") else {
        panic!("the worktree is asked for")
    };
    app.send(InputKind::Done {
        order: id.clone(),
        op,
        result: OpResult::Failed {
            detail: "disk full".into(),
        },
    });
    assert!(matches!(
        app.condition(&id),
        Condition::Failed(FailReason::Effect { .. })
    ));
    app.hold.clear();
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn without_a_gate_plan_no_build_begins() {
    let mut app = App::new(Stage::One, support::rules(""));
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::NoGatePlan));
    assert_eq!(app.asked("start_session"), 0);
}

#[test]
fn changed_gate_commands_wait_for_a_confirmation() {
    let workflow = ide_factory::workflow::parse(WORKFLOW).unwrap().workflow;
    let unconfirmed = ide_factory::workflow::rules::Rules::new(
        &ide_factory::workflow::UserDefaults::default(),
        Some(&workflow),
        &ide_factory::workflow::rules::Confirmations::default(),
    );
    let hash = unconfirmed.gate_hash().to_string();
    let mut app = App::new(Stage::One, unconfirmed);
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::GateCommandsChanged));
    app.send(InputKind::ConfirmGates("other".into()));
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::StaleConfirmation)
    ));
    app.send(InputKind::ConfirmGates(hash.clone()));
    assert!(app.effects.contains(&Effect::Confirm { hash }));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn an_invalid_workflow_file_starts_no_new_station_until_fixed() {
    let mut app = app();
    let errors = ide_factory::workflow::parse("[limits]\nbogus = 1\n").unwrap_err();
    let invalid = ide_factory::workflow::rules::Rules::invalid(&ide_factory::workflow::UserDefaults::default(), errors);
    app.send(InputKind::Rules(Box::new(invalid)));
    let id = app.order_with("o1", |_| {});
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::Sheet { .. })
    ));
    assert!(app.factory.order(&id).is_none());
    app.send(InputKind::Rules(Box::new(rules(WORKFLOW))));
    let id = app.order("o1");
    app.hold.insert("send_prompt");
    app.settle();
    let errors = ide_factory::workflow::parse("[limits]\nbogus = 1\n").unwrap_err();
    let invalid = ide_factory::workflow::rules::Rules::invalid(&ide_factory::workflow::UserDefaults::default(), errors);
    app.send(InputKind::Rules(Box::new(invalid)));
    app.hold.clear();
    app.settle();
    assert_eq!(app.station(&id), Station::Review);
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::InvalidWorkflow));
    app.send(InputKind::Rules(Box::new(rules(WORKFLOW))));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_secret_in_a_turn_is_not_committed_and_goes_back_to_the_builder() {
    let mut app = app();
    app.scans.push_back(ide_factory::gates::SecretScan::Found {
        count: 1,
        places: vec!["src/lib.rs:3".into()],
    });
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.get(&id).turns_in(Station::Build), 2);
    assert_eq!(app.asked("commit"), 1);
}

#[test]
fn a_missing_scanner_is_the_first_deviation() {
    let mut app = app();
    app.scans.extend([
        ide_factory::gates::SecretScan::NotInstalled,
        ide_factory::gates::SecretScan::NotInstalled,
    ]);
    let id = app.order("o1");
    app.settle();
    let deviations = app.get(&id).evidence().expect("sealed").deviations().to_vec();
    assert_eq!(deviations[0], ide_factory::evidence::Deviation::SecretsNotChecked);
}

#[test]
fn an_unknown_order_is_refused() {
    let mut app = app();
    app.send(InputKind::Approve(ide_factory::OrderId("nope".into())));
    assert!(matches!(
        app.refusals.last().map(|r| &r.why),
        Some(Refused::UnknownOrder)
    ));
}
