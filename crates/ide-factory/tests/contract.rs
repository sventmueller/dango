//! The builder's word on the criteria before it builds (E178): it confirms them or objects in a fixed form, an objection
//! makes the order wait for the user, who overrules it, changes the criteria or aborts; and later changes of an order's
//! issue (E176), which change the order only on the user's word.

mod support;

use ide_factory::contract::{CONFIRMED, CriteriaReply};
use ide_factory::effect::Effect;
use ide_factory::event::{Event, TurnOutcome};
use ide_factory::evidence::Invalidation;
use ide_factory::intake::{IssueOrigin, issue_order};
use ide_factory::log::read;
use ide_factory::options::NewOrder;
use ide_factory::{Condition, Factory, InputKind, OrderId, Refused, SessionEvent, Stage, Stamp, Station, WaitReason};
use support::{App, WORKFLOW, rules};

const OBJECTION: &str = "I will not start.\n\nCRITERIA OBJECTION\n- [K1] cannot be checked without a sample file\n";

fn app() -> App {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.insert("send_prompt");
    app
}

fn events(app: &App, id: &OrderId) -> Vec<Event> {
    read(&app.logs[id]).entries.into_iter().map(|e| e.event).collect()
}

/// Settles, then ends the one prompt that waits with `answer`; the prompt's text.
fn answer(app: &mut App, answer: &str) -> String {
    app.settle();
    let Some(Effect::SendPrompt {
        order,
        session,
        prompt,
        text,
        ..
    }) = app.take("send_prompt")
    else {
        panic!("a prompt waits");
    };
    let ended = SessionEvent::TurnEnded {
        prompt: Some(prompt),
        outcome: TurnOutcome::Succeeded,
        answer: Some(answer.to_string()),
    };
    app.send(InputKind::Session(order, session, ended));
    app.settle();
    text
}

fn objection_text() -> String {
    "- [K1] cannot be checked without a sample file".to_string()
}

#[test]
fn the_first_prompt_asks_for_the_builder_s_word_and_a_confirmation_lets_it_build() {
    let mut app = app();
    let id = app.order("o1");
    let prompt = answer(&mut app, "Done.\n\nCRITERIA CONFIRMED");
    assert!(prompt.contains(CONFIRMED), "{prompt}");
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(!app.get(&id).contract_due());
    assert!(events(&app, &id).contains(&Event::CriteriaReplied {
        reply: CriteriaReply::Confirmed
    }));
}

#[test]
fn a_reply_without_the_word_counts_as_missing_and_the_order_goes_on() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, "Done.");
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(events(&app, &id).contains(&Event::CriteriaReplied {
        reply: CriteriaReply::Missing
    }));
}

#[test]
fn an_objection_waits_with_its_text_and_commits_nothing() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, OBJECTION);
    assert_eq!(
        app.condition(&id),
        Condition::Waiting(WaitReason::CriteriaObjection { text: objection_text() })
    );
    assert_eq!(app.station(&id), Station::Build);
    assert_eq!(app.asked("commit"), 0, "the objection's turn is not committed");
    assert!(app.effects.iter().any(|e| matches!(e, Effect::Notify { .. })));
    assert!(app.get(&id).contract_due());
}

#[test]
fn weiter_overrules_the_objection_and_the_builder_carries_out_the_criteria_as_they_are() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, OBJECTION);
    app.send(InputKind::Continue(id.clone()));
    let prompt = answer(&mut app, "Done.");
    assert!(prompt.contains("keeps the acceptance criteria"), "{prompt}");
    assert!(
        !prompt.contains(CONFIRMED),
        "an overruled objection is not asked again: {prompt}"
    );
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(events(&app, &id).contains(&Event::ObjectionOverruled));
}

#[test]
fn changed_criteria_after_an_objection_ask_for_the_builder_s_word_again() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, OBJECTION);
    app.send(InputKind::ChangeCriteria(
        id.clone(),
        "exports CSV from the sample in tests/data\n".into(),
    ));
    assert_eq!(app.condition(&id), Condition::Running);
    let prompt = answer(&mut app, "CRITERIA CONFIRMED");
    assert!(prompt.contains(CONFIRMED) && prompt.contains("tests/data"), "{prompt}");
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(
        app.get(&id).criteria()[0].text(),
        "exports CSV from the sample in tests/data"
    );
}

#[test]
fn an_objection_can_end_in_an_abort() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, OBJECTION);
    app.send(InputKind::Abort(id.clone()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Aborted);
}

#[test]
fn an_objection_comes_back_after_a_restart() {
    let mut app = app();
    let id = app.order("o1");
    answer(&mut app, OBJECTION);
    let logs: Vec<String> = app.logs.values().cloned().collect();
    let restored = Factory::restore(Stage::One, rules(WORKFLOW), &logs, Stamp::new(5, app.wall + 1000));
    assert!(restored.problems.is_empty(), "{:?}", restored.problems);
    let order = restored.factory.order(&id).expect("restored");
    assert_eq!(
        *order.condition(),
        Condition::Waiting(WaitReason::CriteriaObjection { text: objection_text() })
    );
    assert!(order.contract_due());
}

const BODY: &str = "Opening an empty file crashes.\n\n- [ ] an empty file opens\n- [ ] no panic\n";
const CHANGED: &str = "Opening an empty or tiny file crashes.\n\n- [ ] an empty file opens\n- [ ] a tiny file opens\n";

fn issue() -> NewOrder {
    let origin = IssueOrigin::freeze("https://github.com/acme/widget/issues/3", 3, Some("octo"), BODY);
    issue_order(origin, "Crash on an empty file")
}

fn issue_events(app: &App, id: &OrderId) -> usize {
    events(app, id)
        .iter()
        .filter(|e| matches!(e, Event::IssueChanged { .. }))
        .count()
}

#[test]
fn a_changed_issue_is_noted_once_and_changes_nothing_until_the_user_takes_it() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let id = app.order_with("o1", |o| *o = issue());
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::SpecApproval));
    app.send(InputKind::IssueRead {
        order: id.clone(),
        body: BODY.into(),
    });
    assert_eq!(issue_events(&app, &id), 0, "an unchanged issue is not noted");
    for _ in 0..2 {
        app.send(InputKind::IssueRead {
            order: id.clone(),
            body: CHANGED.into(),
        });
    }
    assert_eq!(issue_events(&app, &id), 1, "the same change is noted once");
    assert_eq!(app.get(&id).issue_update(), Some(CHANGED));
    assert!(
        app.get(&id).goal().starts_with("Opening an empty file"),
        "nothing changes before the click"
    );
    app.send(InputKind::IssueRead {
        order: id.clone(),
        body: BODY.into(),
    });
    assert_eq!(
        app.get(&id).issue_update(),
        None,
        "an issue changed back has nothing to take"
    );
    app.send(InputKind::TakeIssueChange(id.clone()));
    assert!(
        app.refusals
            .iter()
            .any(|r| r.order.as_ref() == Some(&id) && r.why == Refused::NotNow)
    );
}

#[test]
fn taking_the_change_at_g1_updates_goal_criteria_and_specification() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let id = app.order_with("o1", |o| *o = issue());
    app.settle();
    app.send(InputKind::IssueRead {
        order: id.clone(),
        body: CHANGED.into(),
    });
    app.send(InputKind::TakeIssueChange(id.clone()));
    app.settle();
    let order = app.get(&id);
    assert!(order.goal().starts_with("Opening an empty or tiny file"));
    let texts: Vec<&str> = order.criteria().iter().map(|c| c.text()).collect();
    assert_eq!(texts, ["an empty file opens", "a tiny file opens"]);
    assert!(order.spec().is_some_and(|s| s.contains("a tiny file opens")));
    assert!(order.options().issue.as_ref().is_some_and(|o| o.unchanged(CHANGED)));
    assert_eq!(order.issue_update(), None);
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::SpecApproval));
}

#[test]
fn taking_the_change_after_the_build_voids_the_evidence_and_builds_again() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let id = app.order_with("o1", |o| *o = issue());
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    let before = app.fingerprint(&id);
    app.send(InputKind::IssueRead {
        order: id.clone(),
        body: CHANGED.into(),
    });
    assert!(
        app.get(&id).evidence().is_some_and(|e| e.invalid().is_none()),
        "a noted change voids nothing"
    );
    app.hold.insert("send_prompt");
    app.send(InputKind::TakeIssueChange(id.clone()));
    assert!(events(&app, &id).contains(&Event::Invalidated {
        why: Invalidation::Criteria
    }));
    let prompt = answer(&mut app, "CRITERIA CONFIRMED");
    assert!(
        prompt.contains("issue changed") && prompt.contains("a tiny file opens"),
        "{prompt}"
    );
    assert!(
        prompt.contains(CONFIRMED),
        "changed criteria ask for the builder's word again"
    );
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::Acceptance));
    assert_ne!(app.fingerprint(&id), before);
}
