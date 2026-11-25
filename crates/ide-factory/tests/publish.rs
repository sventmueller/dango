//! Publishing an order on the user's click (E177): G2 with the branch kept, then the scan of everything that goes out,
//! push, merge request and the issue's comment, each a logged operation; a failure stops it with its reason and a
//! retry goes on where it stopped; a restart verifies what was begun and sends nothing until the next click (C66).

mod support;

use ide_factory::effect::Effect;
use ide_factory::event::{OpResult, Phase};
use ide_factory::gates::SecretScan;
use ide_factory::intake::{IssueOrigin, issue_order};
use ide_factory::publish::{ChangeRef, Outgoing, PublishFailure, PublishStep, marker};
use ide_factory::{Factory, InputKind, OrderId, Refused, Stage, Stamp, Station};
use support::{App, WORKFLOW, rules};

const PUBLISHING: [&str; 4] = ["scan_outgoing", "push", "open_change", "comment_issue"];

const PAGE: &str = "https://github.com/acme/widget/pull/32";

fn texts() -> Outgoing {
    Outgoing::new(
        "Crash on an empty file",
        "Goal and evidence",
        "Accepted, as merge request {change}",
    )
}

/// An app whose publishing effects wait for the test to answer them.
fn app() -> App {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    app.hold.extend(PUBLISHING);
    app
}

/// An order from issue #3 that waits for G2.
fn from_issue(app: &mut App) -> OrderId {
    let origin = IssueOrigin::freeze(
        "https://github.com/acme/widget/issues/3",
        3,
        Some("octo"),
        "It crashes.\n\n- [ ] an empty file opens\n",
    );
    let id = app.order_with("o1", |o| *o = issue_order(origin, "Crash on an empty file"));
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    id
}

/// G2 in the same click as publishing.
fn accept_as_change(app: &mut App, id: &OrderId) {
    let fingerprint = Some(app.fingerprint(id));
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint,
        rejected: Vec::new(),
        texts: texts(),
    });
    app.settle();
}

fn done(app: &mut App, effect: &Effect, result: OpResult) {
    let (order, op) = match effect {
        Effect::Push { order, op, .. }
        | Effect::OpenChange { order, op, .. }
        | Effect::CommentIssue { order, op, .. } => (order.clone(), op.clone()),
        other => panic!("not a publishing step: {other:?}"),
    };
    app.send(InputKind::Done { order, op, result });
}

fn scanned(app: &mut App, id: &OrderId, scan: SecretScan) {
    let Some(Effect::ScanOutgoing { run_id, .. }) = app.take("scan_outgoing") else {
        panic!("the outgoing text is scanned first")
    };
    app.send(InputKind::Scanned(id.clone(), run_id, scan));
}

fn change() -> OpResult {
    OpResult::ChangeOpened {
        change: ChangeRef::new(32, PAGE, false),
    }
}

#[test]
fn accepting_as_a_merge_request_scans_pushes_opens_and_comments_the_issue() {
    let mut app = app();
    let id = from_issue(&mut app);
    let accepted = app.get(&id).evidence().unwrap().commit().to_string();
    accept_as_change(&mut app, &id);
    assert_eq!(app.station(&id), Station::Done);
    assert!(app.get(&id).branch_kept() && !app.get(&id).taken_over());
    let Some(Effect::ScanOutgoing { text, .. }) = app
        .pending
        .iter()
        .find(|e| matches!(e, Effect::ScanOutgoing { .. }))
        .cloned()
    else {
        panic!("no scan of the outgoing text: {:?}", app.pending);
    };
    assert!(
        text.contains("Crash on an empty file") && text.contains("Goal and evidence"),
        "{text}"
    );
    assert!(text.contains(&marker(&id)), "the marker is scanned with the rest");
    scanned(&mut app, &id, SecretScan::Clean);
    let push = app.take("push").expect("the push");
    let Effect::Push { branch, commit, .. } = &push else {
        unreachable!()
    };
    assert_eq!((branch.as_str(), commit.as_str()), ("order/o1", accepted.as_str()));
    assert!(app.get(&id).publishing());
    done(&mut app, &push, OpResult::Pushed);
    let open = app.take("open_change").expect("the merge request");
    let Effect::OpenChange { base, title, body, .. } = &open else {
        unreachable!()
    };
    assert_eq!((base.as_str(), title.as_str()), ("main", "Crash on an empty file"));
    assert!(
        body.starts_with("Goal and evidence") && body.ends_with(&marker(&id)),
        "{body}"
    );
    done(&mut app, &open, change());
    let comment = app.take("comment_issue").expect("the issue's comment");
    let Effect::CommentIssue { issue, body, .. } = &comment else {
        unreachable!()
    };
    assert_eq!(*issue, 3);
    assert!(
        body.starts_with(&format!("Accepted, as merge request {PAGE}")),
        "{body}"
    );
    assert!(body.ends_with(&marker(&id)));
    done(
        &mut app,
        &comment,
        OpResult::Commented {
            url: "https://github.com/acme/widget/issues/3#issuecomment-1".into(),
        },
    );
    let order = app.get(&id);
    assert!(order.published() && !order.publishing());
    assert_eq!(order.publication().unwrap().change().map(ChangeRef::number), Some(32));
    assert_eq!(order.open_operations(), 0);
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    assert!(
        app.refusals.iter().any(|r| r.why == Refused::NotNow),
        "nothing goes out twice"
    );
    let log = &app.logs[&id];
    for action in [
        "\"action\":\"push\"",
        "\"action\":\"open_change\"",
        "\"action\":\"comment_issue\"",
    ] {
        assert_eq!(log.matches(action).count(), 1, "{action} begun once");
    }
}

#[test]
fn an_order_without_an_issue_is_published_after_keeping_its_branch_and_gets_no_comment() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let fingerprint = app.fingerprint(&id);
    app.send(InputKind::KeepBranch {
        order: id.clone(),
        fingerprint,
        rejected: Vec::new(),
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(
        app.asked("scan_outgoing"),
        0,
        "keeping the branch alone publishes nothing"
    );
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    scanned(&mut app, &id, SecretScan::Clean);
    let push = app.take("push").unwrap();
    done(&mut app, &push, OpResult::Pushed);
    let open = app.take("open_change").unwrap();
    done(&mut app, &open, change());
    assert_eq!(app.asked("comment_issue"), 0);
    assert!(app.get(&id).published());
}

#[test]
fn a_missing_gitleaks_or_a_secret_sends_nothing() {
    for (scan, failure) in [
        (SecretScan::NotInstalled, PublishFailure::ScanUnavailable),
        (
            SecretScan::Found {
                count: 2,
                places: vec!["3".into()],
            },
            PublishFailure::SecretsFound { count: 2 },
        ),
    ] {
        let mut app = app();
        let id = from_issue(&mut app);
        accept_as_change(&mut app, &id);
        scanned(&mut app, &id, scan);
        assert_eq!(
            app.asked("push") + app.asked("open_change") + app.asked("comment_issue"),
            0
        );
        let order = app.get(&id);
        assert_eq!(order.publication().unwrap().failure(), Some(&failure));
        assert_eq!(*order.phase(), Phase::Idle);
    }
}

#[test]
fn a_failed_step_names_its_reason_and_a_retry_goes_on_where_it_stopped() {
    let mut app = app();
    let id = from_issue(&mut app);
    accept_as_change(&mut app, &id);
    scanned(&mut app, &id, SecretScan::Clean);
    let push = app.take("push").unwrap();
    done(&mut app, &push, OpResult::Pushed);
    let open = app.take("open_change").unwrap();
    let detail = "the token for github.com lacks the scope pull_requests=write".to_string();
    done(&mut app, &open, OpResult::Failed { detail: detail.clone() });
    let failure = PublishFailure::Step {
        step: PublishStep::Change,
        detail,
    };
    assert_eq!(app.get(&id).publication().unwrap().failure(), Some(&failure));
    assert_eq!(
        app.station(&id),
        Station::Done,
        "a failed merge request does not fail the order"
    );
    assert!(app.get(&id).publication().unwrap().pushed());
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    assert_eq!(app.get(&id).publication().unwrap().failure(), None);
    scanned(&mut app, &id, SecretScan::Clean);
    assert_eq!(app.asked("push"), 1, "the branch is not pushed again");
    assert!(app.take("open_change").is_some(), "the retry opens the merge request");
}

#[test]
fn a_restart_verifies_a_begun_step_and_sends_nothing_until_the_next_click() {
    let mut app = app();
    let id = from_issue(&mut app);
    accept_as_change(&mut app, &id);
    scanned(&mut app, &id, SecretScan::Clean);
    let push = app.take("push").unwrap();
    done(&mut app, &push, OpResult::Pushed);
    assert!(
        app.take("open_change").is_some(),
        "begun, and the app ends before its answer"
    );
    let logs: Vec<String> = app.logs.values().cloned().collect();
    let restored = Factory::restore(Stage::One, rules(WORKFLOW), &logs, Stamp::new(5, app.wall + 1000));
    let verify = restored.step.effects.iter().find_map(|e| match e {
        Effect::Verify { op, target, .. } => Some((op.clone(), target.clone())),
        _ => None,
    });
    let (op, target) = verify.expect("the merge request is verified, not opened again");
    assert!(target.on_service());
    assert!(
        !restored
            .step
            .effects
            .iter()
            .any(|e| matches!(e, Effect::OpenChange { .. } | Effect::ScanOutgoing { .. }))
    );
    let mut next = App::with(restored.factory);
    next.hold.extend(PUBLISHING);
    next.wall = app.wall + 2000;
    next.send(InputKind::Verified {
        order: id.clone(),
        op,
        reached: Some(OpResult::ChangeOpened {
            change: ChangeRef::new(32, PAGE, true),
        }),
    });
    let order = next.get(&id);
    assert!(order.publication().unwrap().change().is_some_and(ChangeRef::reused));
    assert!(!order.published() && !order.publishing());
    assert_eq!(next.asked("comment_issue"), 0, "the comment waits for the user's click");
    next.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    scanned(&mut next, &id, SecretScan::Clean);
    assert!(next.take("comment_issue").is_some());
    assert_eq!(next.asked("push") + next.asked("open_change"), 0);
}

#[test]
fn publishing_waits_for_g2_and_a_kept_branch() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    assert!(app.refusals.iter().any(|r| r.why == Refused::NotNow));
    app.accept(&id);
    app.settle();
    assert!(app.get(&id).taken_over());
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint: None,
        rejected: Vec::new(),
        texts: texts(),
    });
    assert_eq!(app.asked("scan_outgoing"), 0, "a taken-over order is not published");
}

fn section() -> Vec<ide_factory::HunkId> {
    vec![ide_factory::HunkId("src/lib.rs#2".into())]
}

#[test]
fn keeping_the_branch_with_a_section_rejected_reviews_what_is_left_first() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let fingerprint = app.fingerprint(&id);
    app.send(InputKind::KeepBranch {
        order: id.clone(),
        fingerprint: fingerprint.clone(),
        rejected: section(),
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert!(app.get(&id).branch_kept() && !app.get(&id).taken_over());
    assert_eq!(app.asked("reject_hunks"), 1);
    assert_eq!(app.asked("run_gate"), 2, "the gates ran for what is left");
    let sealed = app.get(&id).evidence().unwrap().fingerprint().to_string();
    assert_ne!(sealed, fingerprint);
    assert_eq!(app.get(&id).accepted(), Some(sealed.as_str()));
}

#[test]
fn accepting_as_a_merge_request_with_a_section_rejected_publishes_what_is_left() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let fingerprint = Some(app.fingerprint(&id));
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint,
        rejected: section(),
        texts: texts(),
    });
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert_eq!(app.asked("reject_hunks"), 1);
    let accepted = app.get(&id).evidence().unwrap().commit().to_string();
    scanned(&mut app, &id, SecretScan::Clean);
    let Some(Effect::Push { commit, .. }) = app.take("push") else {
        panic!("no push");
    };
    assert_eq!(commit, accepted, "the commit without the rejected section goes out");
}

#[test]
fn a_partial_acceptance_voided_before_it_closes_drops_keeping_and_publishing() {
    let mut app = app();
    let id = app.order("o1");
    app.settle();
    let fingerprint = Some(app.fingerprint(&id));
    app.hold.insert("run_gate");
    app.send(InputKind::Publish {
        order: id.clone(),
        fingerprint,
        rejected: section(),
        texts: texts(),
    });
    app.settle();
    assert!(app.get(&id).branch_kept() && app.get(&id).publication().is_some());
    app.pending.clear();
    app.hold.remove("run_gate");
    app.send(InputKind::BackToBuilder(id.clone(), "Keep the header.".into()));
    app.settle();
    assert!(!app.get(&id).branch_kept() && app.get(&id).publication().is_none());
    assert_eq!(app.get(&id).accepted(), None);
    app.accept(&id);
    app.settle();
    assert_eq!(app.station(&id), Station::Done);
    assert!(
        app.get(&id).taken_over(),
        "the plain G2 that followed takes the order over"
    );
    assert_eq!(app.asked("scan_outgoing"), 0);
}
