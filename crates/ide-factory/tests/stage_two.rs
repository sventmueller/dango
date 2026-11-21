//! Stage 2: the planner and G1 (E178), the reviewer from another vendor (E162, E171), its structured verdict and the
//! loop back to the builder (E179), and the user's interventions (E184).

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

mod support;

use ide_factory::effect::Effect;
use ide_factory::event::Phase;
use ide_factory::evidence::Deviation;
use ide_factory::options::RiskChoice;
use ide_factory::routing::{Agent, AgentStatus, RankEntry, ReviewerRule};
use ide_factory::workflow::UserDefaults;
use ide_factory::workflow::rules::{Confirmations, Loosening, Rules};
use ide_factory::{Condition, InputKind, RiskClass, Role, SessionEvent, Stage, Station, WaitReason};
use support::{App, WORKFLOW, rules, rules_with};

fn app() -> App {
    App::new(Stage::Two, rules(WORKFLOW))
}

#[test]
fn the_planner_writes_the_spec_and_g1_approves_it() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.station(&id), Station::Spec);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::SpecApproval));
    assert!(app.effects.iter().any(|e| matches!(e, Effect::WriteSpec { .. })));
    assert_eq!(
        app.get(&id).options().scope,
        ["src/**"],
        "the planner's scope, as no scope was given"
    );
    assert!(app.effects.iter().any(|e| matches!(
        e,
        Effect::StartSession {
            role: Role::Planner,
            read_only: true,
            ..
        }
    )));
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    let reviewer = app.get(&id).session(Role::Reviewer).expect("a reviewer");
    assert_eq!(reviewer.entry().agent, Agent::Codex, "another vendor than the builder");
    assert!(!app.get(&id).deviations().contains(&Deviation::SameVendorReview));
    let review = app.effects.iter().find_map(|e| match e {
        Effect::SendPrompt { text, schema, .. } if schema.is_some() => Some(text.clone()),
        _ => None,
    });
    let review = review.expect("the reviewer's prompt carries the schema");
    assert!(review.contains("untrusted") && review.contains("[K1]"), "{review}");
}

#[test]
fn a_comment_on_the_spec_goes_back_to_the_planner() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::CommentSpec(id.clone(), "Also handle quotes.".into()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::SpecApproval));
    let plans = app
        .effects
        .iter()
        .filter(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("Write the specification")))
        .count();
    assert_eq!(plans, 2);
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("Also handle quotes.")))
    );
}

fn skipping_rules() -> Rules {
    let text = format!("{WORKFLOW}[spec]\nskip_approval = true\n");
    let workflow = ide_factory::workflow::parse(&text).unwrap().workflow;
    let first = Rules::new(&UserDefaults::default(), Some(&workflow), &Confirmations::default());
    let confirmed = Confirmations::new([first.gate_hash().to_string(), Loosening::SkipSpecApproval.hash()]);
    Rules::new(&UserDefaults::default(), Some(&workflow), &confirmed)
}

#[test]
fn g1_is_skipped_for_class_low_where_the_project_allows_it() {
    let mut app = App::new(Stage::Two, skipping_rules());
    let id = app.order_with("o1", |o| {
        o.risk = RiskChoice::Fixed(RiskClass::Low);
        o.skip_spec_approval = true;
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(app.get(&id).g1_skipped());
    assert!(!app.get(&id).approved());
}

#[test]
fn the_reviewer_sends_unmet_criteria_back_to_the_builder() {
    let mut app = app();
    app.verdicts
        .push_back(r#"{"criteria":[{"id":"K1","verdict":"not_met","reason":"no header"}],"findings":[{"file":"src/csv.rs","line":4,"severity":"medium","text":"missing header"}]}"#.into());
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(app.get(&id).review_log().len(), 2);
    assert_eq!(app.get(&id).review_rounds(), 1);
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("missing header")))
    );
    let reviewers = app
        .effects
        .iter()
        .filter(|e| {
            matches!(
                e,
                Effect::StartSession {
                    role: Role::Reviewer,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(reviewers.last(), Some(Effect::StartSession { resume: Some(_), .. })),
        "the second round resumes the reviewer's session (A15)"
    );
}

#[test]
fn an_unreadable_verdict_goes_to_the_user() {
    let mut app = app();
    app.verdicts.push_back("Looks good to me!".into());
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Unclear));
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

fn only_claude(app: &mut App) {
    let agents = [Agent::Claude]
        .into_iter()
        .map(|a| (a.clone(), AgentStatus::ready(&a)))
        .collect();
    app.send(InputKind::Agents(agents));
}

#[test]
fn without_another_vendor_the_rule_required_waits_for_the_user() {
    let mut app = app();
    only_claude(&mut app);
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Review);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::NoReviewer));
}

#[test]
fn under_the_rule_preferred_the_same_vendor_reviews_and_the_evidence_says_so() {
    let mut defaults = UserDefaults::default();
    defaults.set_reviewer_rule(ReviewerRule::Preferred);
    let mut app = App::new(Stage::Two, rules_with(&defaults, WORKFLOW));
    only_claude(&mut app);
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    let deviations = app.get(&id).evidence().unwrap().deviations().to_vec();
    assert!(deviations.contains(&Deviation::SameVendorReview));
}

#[test]
fn a_model_switch_of_the_builder_counts_for_the_reviewer_rule() {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order_with("o1", |o| {
        o.casting
            .insert(Role::Builder, RankEntry::new(Agent::Own).model("gpt-5"));
    });
    app.settle();
    let planner = app.take("send_prompt").unwrap();
    let reply = app.reply(&planner).unwrap();
    app.send(reply);
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    let session = app.turn_session(&id);
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::ModelSwitched {
            model: "claude-sonnet-4-5".into(),
        },
    ));
    app.hold.clear();
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::NoReviewer));
    assert!(
        app.get(&id)
            .deviations()
            .iter()
            .any(|d| matches!(d, Deviation::ModelSwitched { .. }))
    );
}

#[test]
fn interventions_are_deviations_and_the_gate_waits_for_the_queue() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.hold.insert("send_prompt");
    app.settle();
    let session = app.turn_session(&id);
    let prompt = app.get(&id).turn().unwrap().prompt();
    app.send(InputKind::Session(
        id.clone(),
        session.clone(),
        SessionEvent::Intervened,
    ));
    let queued = |queued| SessionEvent::Activity {
        running: true,
        tool_since: None,
        subagents: 0,
        background: 0,
        queued,
    };
    app.send(InputKind::Session(id.clone(), session.clone(), queued(1)));
    app.send(InputKind::Session(
        id.clone(),
        session.clone(),
        SessionEvent::TurnEnded {
            prompt: Some(prompt),
            outcome: ide_factory::event::TurnOutcome::Succeeded,
            answer: None,
        },
    ));
    assert!(
        matches!(app.get(&id).phase(), Phase::Turn { .. }),
        "the user's message is still queued"
    );
    app.send(InputKind::Session(id.clone(), session, queued(0)));
    assert_eq!(*app.get(&id).phase(), Phase::TurnScan);
    app.hold.clear();
    app.pending.retain(|e| support::kind(e) != "send_prompt");
    app.settle();
    let deviations = app.get(&id).evidence().unwrap().deviations().to_vec();
    assert!(deviations.contains(&Deviation::Interventions { count: 1 }));
}

#[test]
fn an_order_keeps_the_stage_it_was_created_in() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("prepare_worktree");
    let first = app.order("o1");
    app.factory.set_stage(Stage::Two);
    let second = app.order("o2");
    assert_eq!(app.get(&first).stage(), Stage::One);
    assert_eq!(app.get(&second).stage(), Stage::Two);
    assert!(app.get(&first).g1_skipped(), "stage 1 skips the specification");
    assert_eq!(app.station(&first), Station::Build);
}

#[test]
fn two_review_rounds_that_do_not_pass_escalate_to_the_user() {
    let mut app = App::new(Stage::Two, rules(&format!("{WORKFLOW}[limits]\nreview_rounds = 2\n")));
    let not_met = r#"{"criteria":[{"id":"K1","verdict":"not_met","reason":"no header"}],"findings":[]}"#;
    app.verdicts.extend([not_met.to_string(), not_met.to_string()]);
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.effects.clear();
    app.settle();
    assert_eq!(app.station(&id), Station::Review);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::ReviewRoundsSpent));
    assert_eq!(app.get(&id).review_rounds(), 2);
    assert_eq!(app.get(&id).review_log().len(), 2);
    let reworks = app
        .effects
        .iter()
        .filter(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("did not accept")))
        .count();
    assert_eq!(
        reworks, 1,
        "the first round goes back to the builder, the second to the user"
    );
    assert!(app.effects.iter().any(|e| matches!(
        e,
        Effect::Notify {
            notice: ide_factory::effect::Notice::Waiting,
            ..
        }
    )));
    app.send(InputKind::Continue(id.clone()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    let evidence = app.get(&id).evidence().expect("sealed");
    assert_eq!(evidence.rounds().len(), 3, "every round stands in the evidence");
}
