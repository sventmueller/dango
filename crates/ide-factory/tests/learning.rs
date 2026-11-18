//! Stage 4 (B47): the figures of the order logs (E180), the tie-break by first-pass rate in routing (E170), the
//! scout's rule proposals and the recurring orders of the workflow file.

mod support;

use ide_factory::evidence::Deviation;
use ide_factory::learning::{PROPOSAL_SCOPE, proposal_due, proposal_order};
use ide_factory::metrics::{MIN_ORDERS, Metrics};
use ide_factory::routing::{Agent, RankEntry};
use ide_factory::workflow::recurring::Interval;
use ide_factory::workflow::rules::{Confirmations, Loosening, Rules};
use ide_factory::workflow::{UserDefaults, WorkflowErrorKind, parse};
use ide_factory::{Condition, InputKind, OrderId, RiskClass, Role, SessionEvent, Stage, Station, WaitReason};
use support::{App, WORKFLOW, rules};

/// A verdict that finds criterion K1 not met, with one finding in `words`.
fn not_met(words: &str) -> String {
    format!(
        r#"{{"criteria":[{{"id":"K1","verdict":"not_met","reason":"missing"}}],"findings":[{{"file":"src/lib.rs","line":3,"severity":"medium","text":"{words}"}}]}}"#
    )
}

/// Runs order `id`, built by `builder`, through G1 to the acceptance, back to the builder `reworks` times.
fn history(app: &mut App, id: &str, builder: Agent, reworks: usize, finding: &str) -> OrderId {
    let id = app.order_with(id, |sheet| {
        sheet.casting.insert(Role::Builder, RankEntry::new(builder));
    });
    app.settle();
    app.send(InputKind::Approve(id.clone()));
    for _ in 0..reworks {
        app.verdicts.push_back(not_met(finding));
    }
    app.settle();
    id
}

/// An app in stage 2 whose Claude builds `claude` orders with one rework each and whose Codex builds `codex` orders
/// that pass at once.
fn with_history(claude: u32, codex: u32) -> App {
    let mut app = App::new(Stage::Two, rules(WORKFLOW));
    for n in 0..claude {
        let id = history(
            &mut app,
            &format!("claude-{n}"),
            Agent::Claude,
            1,
            "unwrap on user input",
        );
        assert_eq!(app.station(&id), Station::Acceptance);
    }
    for n in 0..codex {
        let id = history(&mut app, &format!("codex-{n}"), Agent::Codex, 0, "");
        assert_eq!(app.station(&id), Station::Acceptance);
    }
    app
}

#[test]
fn the_figures_count_every_order_and_rate_only_the_eligible_ones() {
    let mut app = with_history(2, 1);
    app.hold.insert("send_prompt");
    let helped = app.order("helped");
    app.settle();
    let session = app.turn_session(&helped);
    app.send(InputKind::Session(helped.clone(), session, SessionEvent::Intervened));
    let metrics = Metrics::new(app.factory.orders(), app.wall);
    assert_eq!(
        (metrics.orders(), metrics.eligible()),
        (4, 3),
        "the running order counts, but not for the rate"
    );
    let risk = app.get(&OrderId("claude-0".into())).risk();
    assert_eq!(risk, RiskClass::Low);
    let claude = metrics.rate(Role::Builder, &Agent::Claude, None, risk).expect("a rate");
    assert_eq!((claude.rate, claude.orders), (0.0, 2));
    let codex = metrics.rate(Role::Builder, &Agent::Codex, None, risk).expect("a rate");
    assert_eq!((codex.rate, codex.orders, codex.sound()), (1.0, 1, false));
    assert_eq!(metrics.rate(Role::Builder, &Agent::Codex, None, RiskClass::High), None);
    assert_eq!(
        metrics.rate(Role::Reviewer, &Agent::Codex, None, risk),
        None,
        "only builders have a rate"
    );
    let rounds: u32 = metrics
        .figures()
        .iter()
        .filter(|(k, _)| k.role() == Role::Builder && *k.agent() == Agent::Claude)
        .map(|(_, f)| f.rounds())
        .sum();
    assert_eq!(rounds, 2);
}

#[test]
fn the_better_first_pass_rate_breaks_a_tie_between_builders_and_the_evidence_says_so() {
    let mut app = with_history(MIN_ORDERS, MIN_ORDERS);
    let metrics = Metrics::new(app.factory.orders(), app.wall);
    app.send(InputKind::Metrics(Box::new(metrics)));
    let next = app.order("next");
    app.settle();
    app.send(InputKind::Approve(next.clone()));
    app.settle();
    let order = app.get(&next);
    let builder = order.session(Role::Builder).map(|s| s.entry().agent.clone());
    assert_eq!(
        builder,
        Some(Agent::Codex),
        "Claude ranks first, Codex passes more often"
    );
    let note = |deviations: &[Deviation]| {
        deviations.iter().find_map(|d| match d {
            Deviation::TieBreak {
                role,
                over,
                percent,
                over_percent,
                ..
            } => Some((*role, over.agent.clone(), *percent, *over_percent)),
            _ => None,
        })
    };
    assert_eq!(note(order.deviations()), Some((Role::Builder, Agent::Claude, 100, 0)));
    let evidence = order.evidence().expect("sealed");
    assert_eq!(
        note(evidence.deviations()),
        Some((Role::Builder, Agent::Claude, 100, 0))
    );
}

#[test]
fn without_enough_orders_the_ranked_list_decides() {
    let mut app = with_history(MIN_ORDERS, MIN_ORDERS - 1);
    let metrics = Metrics::new(app.factory.orders(), app.wall);
    app.send(InputKind::Metrics(Box::new(metrics)));
    let next = app.order("next");
    app.settle();
    app.send(InputKind::Approve(next.clone()));
    app.settle();
    let order = app.get(&next);
    assert_eq!(
        order.session(Role::Builder).map(|s| s.entry().agent.clone()),
        Some(Agent::Claude)
    );
    assert!(
        !order
            .deviations()
            .iter()
            .any(|d| matches!(d, Deviation::TieBreak { .. }))
    );
}

#[test]
fn a_rule_proposal_is_due_after_an_escalation_once_findings_recur_in_enough_orders() {
    let mut app = with_history(2, MIN_ORDERS - 2);
    let first = OrderId("claude-0".into());
    assert_eq!(
        proposal_due(app.factory.orders(), app.get(&first)),
        None,
        "not escalated"
    );
    let stuck = history(&mut app, "stuck", Agent::Claude, 3, "unwrap on user input");
    assert_eq!(app.condition(&stuck), Condition::Waiting(WaitReason::ReviewRoundsSpent));
    let found = proposal_due(app.factory.orders(), app.get(&stuck)).expect("a proposal is due");
    assert_eq!(found[0].orders(), 3);
    assert!(found[0].text().contains("unwrap on user input"), "{}", found[0].text());
    let mut fewer = App::new(Stage::Two, rules(WORKFLOW));
    let lonely = history(&mut fewer, "stuck", Agent::Claude, 3, "unwrap on user input");
    assert_eq!(
        proposal_due(fewer.factory.orders(), fewer.get(&lonely)),
        None,
        "too few orders"
    );
    let proposal = proposal_order("Regelvorschlag: 1", "Propose a rule.", &found);
    app.order_with("proposal", |sheet| *sheet = proposal);
    app.settle();
    assert_eq!(
        proposal_due(app.factory.orders(), app.get(&stuck)),
        None,
        "one open proposal at a time"
    );
}

#[test]
fn a_rule_proposal_waits_at_g1_with_the_scouts_specification_even_in_stage_one() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let proposal = proposal_order("Regelvorschlag: 1", "Propose a rule.", &[]);
    let id = app.order_with("proposal", |sheet| *sheet = proposal);
    app.settle();
    let order = app.get(&id);
    assert_eq!(order.station(), Station::Spec);
    assert_eq!(*order.condition(), Condition::Waiting(WaitReason::SpecApproval));
    assert_eq!(
        order.session(Role::Scout).map(|s| s.entry().agent.clone()),
        Some(Agent::Own)
    );
    assert!(order.session(Role::Planner).is_none());
    assert_eq!(
        order.criteria().len(),
        1,
        "the criteria come from the scout's specification"
    );
    assert_eq!(order.options().scope, PROPOSAL_SCOPE);
    app.hold.insert("send_prompt");
    app.send(InputKind::Approve(id.clone()));
    app.settle();
    assert_eq!(app.station(&id), Station::Build);
    assert!(app.get(&id).accepted().is_none(), "nothing is taken over without G2");
}

#[test]
fn recurring_orders_read_from_the_workflow_file_and_apply_once_confirmed() {
    let text = format!(
        "{WORKFLOW}[[recurring]]\ntitle = \"Dead code\"\ngoal = \"Remove dead code.\"\n\
         criteria = [\"make check passes\"]\ninterval = \"14d\"\n"
    );
    let workflow = parse(&text).expect("valid").workflow;
    assert_eq!(workflow.recurring().len(), 1);
    assert_eq!(workflow.recurring()[0].interval(), Interval::Days(14));
    assert_eq!(workflow.recurring()[0].order().criteria, "make check passes");
    let defaults = UserDefaults::default();
    let first = Rules::new(&defaults, Some(&workflow), &Confirmations::default());
    assert!(first.recurring().is_empty(), "not before the user confirmed them");
    let loosening = Loosening::RecurringOrders {
        orders: workflow.recurring().to_vec(),
    };
    assert!(first.pending().contains(&loosening));
    let confirmed = Rules::new(&defaults, Some(&workflow), &Confirmations::new([loosening.hash()]));
    assert_eq!(confirmed.recurring().len(), 1);
    assert_eq!(
        confirmed.hash(),
        first.hash(),
        "recurring orders leave the evidence valid"
    );
    assert!(defaults.rule_proposals(), "rule proposals are on by default");
}

#[test]
fn a_recurring_order_without_its_parts_or_with_a_bad_interval_names_each_error() {
    let text = "[[recurring]]\ntitle = \"A\"\ninterval = \"fortnightly\"\n\n\
                [[recurring]]\ntitle = \"B\"\ngoal = \"g\"\ncriteria = [\"c\"]\ninterval = \"daily\"\n\n\
                [[recurring]]\ntitle = \"B\"\ngoal = \"g\"\ncriteria = [\"c\"]\ninterval = \"weekly\"\n";
    let errors = parse(text).expect_err("invalid");
    let kinds: Vec<(String, WorkflowErrorKind)> = errors.into_iter().map(|e| (e.key, e.kind)).collect();
    assert!(kinds.contains(&("recurring.goal".into(), WorkflowErrorKind::Missing)));
    assert!(kinds.contains(&("recurring.criteria".into(), WorkflowErrorKind::Missing)));
    assert!(kinds.contains(&(
        "recurring.interval".into(),
        WorkflowErrorKind::BadInterval {
            value: "fortnightly".into()
        }
    )));
    assert!(kinds.contains(&(
        "recurring.title".into(),
        WorkflowErrorKind::DuplicateRecurring { title: "B".into() }
    )));
    assert_eq!(kinds.len(), 4, "{kinds:?}");
}
