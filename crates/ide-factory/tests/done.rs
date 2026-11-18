//! The end of an order: why G2 is asked again after a take-over found another state, the reports the take-over keeps,
//! the done order's place in the panel and deleting it.

mod support;

use ide_factory::effect::Effect;
use ide_factory::event::OpResult;
use ide_factory::evidence::Invalidation;
use ide_factory::panel::Panel;
use ide_factory::{Condition, InputKind, OrderId, Refused, Stage, Station, WaitReason};
use support::{App, WORKFLOW, rules};

fn app() -> App {
    App::new(Stage::One, rules(WORKFLOW))
}

fn kept_reports(app: &App, id: &OrderId) -> Option<Vec<String>> {
    app.effects.iter().find_map(|effect| match effect {
        Effect::RemoveWorktree { order, reports, .. } if order == id => Some(reports.clone()),
        _ => None,
    })
}

#[test]
fn a_take_over_onto_another_state_says_why_g2_is_asked_again() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.get(&id).reacceptance(), None, "the first G2 needs no reason");
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
    assert_eq!(app.get(&id).reacceptance(), Some(Invalidation::Base));
    app.assert_replays();
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(app.get(&id).reacceptance(), None);
}

#[test]
fn the_take_over_names_the_reports_to_keep_and_an_abort_keeps_none() {
    let mut app = app();
    let scope = || vec!["src/**".to_string(), "docs/review.md".to_string()];
    let id = app.order_with("o1", |o| o.scope = scope());
    app.settle();
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(kept_reports(&app, &id), Some(vec!["docs/review.md".to_string()]));
    app.hold.insert("send_prompt");
    let aborted = app.order_with("o2", |o| o.scope = scope());
    app.settle();
    app.send(InputKind::Abort(aborted.clone()));
    app.settle();
    assert_eq!(kept_reports(&app, &aborted), Some(Vec::new()));
}

#[test]
fn a_done_order_stays_in_the_panel_until_it_is_deleted_and_an_aborted_one_does_not() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Delete(id.clone()));
    assert!(
        app.refusals
            .iter()
            .any(|r| r.order.as_ref() == Some(&id) && r.why == Refused::NotNow),
        "an open order is not deleted"
    );
    assert!(app.factory.order(&id).is_some());
    app.accept(&id);
    app.settle();
    app.hold.insert("send_prompt");
    let aborted = app.order("o2");
    app.settle();
    app.send(InputKind::Abort(aborted));
    app.settle();
    let panel = Panel::new(&app.factory, app.at());
    let listed: Vec<(Station, Vec<OrderId>)> = panel
        .groups()
        .iter()
        .map(|(station, rows)| (*station, rows.iter().map(|r| r.id().clone()).collect()))
        .collect();
    assert_eq!(listed, [(Station::Done, vec![id.clone()])]);
    assert!(!panel.dot(), "a done order does not wait for the user");
    app.send(InputKind::Delete(id.clone()));
    assert!(app.factory.order(&id).is_none());
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::DeleteOrder { order } if *order == id))
    );
    assert!(Panel::new(&app.factory, app.at()).groups().is_empty());
}
