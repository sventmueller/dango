//! The timing rules of E168 with a set clock: hanging turns, the tool-call and turn limits, retries with a growing
//! wait, the grace after waking, sleep, budgets before each turn and late usage reports, quotas and the wake lock.

mod support;

use ide_factory::budget::{Budget, BudgetScope, UsageReport};
use ide_factory::effect::{Effect, Notice};
use ide_factory::event::{Event, Phase, TurnOutcome};
use ide_factory::evidence::Deviation;
use ide_factory::limits::Overdue;
use ide_factory::log::read;
use ide_factory::quota::{QuotaKey, QuotaObservation, QuotaStatus};
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, FailReason, InputKind, Millis, PauseReason, SessionEvent, Stage, Station, WaitReason};
use support::{App, WORKFLOW, rules};

const SECOND: u64 = 1_000;
const MINUTE: u64 = 60 * SECOND;

fn app() -> App {
    App::new(Stage::One, rules(WORKFLOW))
}

/// An app whose order `o1` has its first turn running, unanswered.
fn running() -> (App, ide_factory::OrderId) {
    let mut app = app();
    app.hold.insert("send_prompt");
    let id = app.order("o1");
    app.settle();
    assert!(matches!(app.get(&id).phase(), Phase::Turn { .. }));
    (app, id)
}

fn tick(app: &mut App) {
    app.send(InputKind::Tick);
}

fn retries(app: &App, id: &ide_factory::OrderId) -> Vec<(u32, bool, u64)> {
    read(&app.logs[id])
        .entries
        .into_iter()
        .filter_map(|e| match e.event {
            Event::Retry {
                attempt,
                counted,
                delay_ms,
            } => Some((attempt, counted, delay_ms)),
            _ => None,
        })
        .collect()
}

fn stopped(app: &App, id: &ide_factory::OrderId) -> Vec<Overdue> {
    read(&app.logs[id])
        .entries
        .into_iter()
        .filter_map(|e| match e.event {
            Event::TurnStopped { why, .. } => why,
            _ => None,
        })
        .collect()
}

#[test]
fn a_silent_turn_hangs_after_five_minutes_and_is_stopped() {
    let (mut app, id) = running();
    app.pass(5 * MINUTE - 10);
    tick(&mut app);
    assert!(stopped(&app, &id).is_empty());
    app.pass(20);
    tick(&mut app);
    assert_eq!(stopped(&app, &id), [Overdue::Stalled]);
    assert_eq!(app.asked("stop_turn"), 1);
    assert_eq!(*app.get(&id).phase(), Phase::Retrying);
    assert_eq!(retries(&app, &id), [(2, true, 10 * SECOND)]);
}

#[test]
fn a_running_tool_call_has_thirty_minutes() {
    let (mut app, id) = running();
    let session = app.turn_session(&id);
    let since = Millis(app.mono);
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::Activity {
            running: true,
            tool_since: Some(since),
            subagents: 0,
            background: 0,
            queued: 0,
        },
    ));
    app.pass(29 * MINUTE);
    tick(&mut app);
    assert!(stopped(&app, &id).is_empty(), "no stall while a tool call runs");
    app.pass(MINUTE + 10);
    tick(&mut app);
    assert_eq!(stopped(&app, &id), [Overdue::ToolCall]);
}

#[test]
fn a_turn_may_take_an_hour_even_with_events() {
    let (mut app, id) = running();
    let session = app.turn_session(&id);
    for _ in 0..12 {
        app.pass(5 * MINUTE);
        app.send(InputKind::Session(
            id.clone(),
            session.clone(),
            SessionEvent::Activity {
                running: true,
                tool_since: None,
                subagents: 1,
                background: 0,
                queued: 0,
            },
        ));
        tick(&mut app);
    }
    assert_eq!(stopped(&app, &id), [Overdue::Turn]);
}

#[test]
fn retries_wait_ten_then_twenty_seconds_and_the_third_failure_fails_the_station() {
    let mut app = app();
    app.turns.extend([
        TurnOutcome::Failed("overloaded".into()),
        TurnOutcome::Failed("overloaded".into()),
        TurnOutcome::ConnectionLost,
    ]);
    let id = app.order("o1");
    app.settle();
    assert_eq!(retries(&app, &id), [(2, true, 10 * SECOND)]);
    app.pass(10 * SECOND - 5);
    tick(&mut app);
    assert_eq!(*app.get(&id).phase(), Phase::Retrying, "not yet due");
    app.pass(10);
    tick(&mut app);
    app.settle();
    assert_eq!(retries(&app, &id), [(2, true, 10 * SECOND), (3, true, 20 * SECOND)]);
    app.pass(20 * SECOND);
    tick(&mut app);
    app.settle();
    assert_eq!(app.condition(&id), Condition::Failed(FailReason::AttemptsSpent));
    assert!(app.effects.iter().any(|e| matches!(
        e,
        Effect::Notify {
            notice: Notice::Failed,
            ..
        }
    )));
}

#[test]
fn failures_in_the_two_minutes_after_waking_do_not_count() {
    let (mut app, id) = running();
    app.send(InputKind::Sleep);
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::Sleep));
    assert!(!app.factory.holds_wake_lock());
    app.sleep_through(30 * MINUTE);
    app.send(InputKind::Wake);
    assert_eq!(app.condition(&id), Condition::Running);
    tick(&mut app);
    assert!(
        stopped(&app, &id).is_empty(),
        "the time asleep does not count towards the stall"
    );
    let session = app.turn_session(&id);
    let prompt = app.get(&id).turn().unwrap().prompt();
    app.pass(MINUTE);
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::TurnEnded {
            prompt: Some(prompt),
            outcome: TurnOutcome::ConnectionLost,
            answer: None,
        },
    ));
    assert_eq!(retries(&app, &id), [(1, false, 10 * SECOND)]);
    assert_eq!(app.get(&id).attempts(), 0);
}

/// An app whose turn has been silent for four minutes when the Mac sleeps for half an hour, with `asleep` letting
/// the time pass on the clocks, and wakes.
fn slept(asleep: impl FnOnce(&mut App)) -> (App, ide_factory::OrderId) {
    let (mut app, id) = running();
    app.pass(4 * MINUTE);
    app.send(InputKind::Sleep);
    asleep(&mut app);
    app.send(InputKind::Wake);
    tick(&mut app);
    assert!(stopped(&app, &id).is_empty(), "the time asleep does not count");
    (app, id)
}

#[test]
fn the_time_asleep_never_counts_towards_a_turn_whichever_clock_counts_it() {
    let (mut mac, id) = slept(|app| app.sleep_through(30 * MINUTE));
    mac.pass(MINUTE + 10);
    tick(&mut mac);
    assert_eq!(
        stopped(&mac, &id),
        [Overdue::Stalled],
        "the silence before the sleep still counts"
    );
    let (mut running_on, id) = slept(|app| app.pass(30 * MINUTE));
    running_on.pass(MINUTE + 10);
    tick(&mut running_on);
    assert_eq!(
        stopped(&running_on, &id),
        [Overdue::Stalled],
        "the turn's clock moved by exactly the sleep"
    );
}

#[test]
fn a_failure_after_the_grace_counts() {
    let (mut app, id) = running();
    app.send(InputKind::Sleep);
    app.send(InputKind::Wake);
    app.pass(2 * MINUTE + 10);
    let session = app.turn_session(&id);
    let prompt = app.get(&id).turn().unwrap().prompt();
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::TurnEnded {
            prompt: Some(prompt),
            outcome: TurnOutcome::Failed("x".into()),
            answer: None,
        },
    ));
    assert_eq!(retries(&app, &id), [(2, true, 10 * SECOND)]);
}

#[test]
fn the_wake_lock_is_held_while_an_order_runs_and_on_battery_only_by_setting() {
    let (mut app, _) = running();
    assert!(app.factory.holds_wake_lock());
    app.send(InputKind::Battery(true));
    assert!(!app.factory.holds_wake_lock());
    let mut defaults = ide_factory::workflow::UserDefaults::default();
    defaults.set_awake_on_battery(true);
    app.send(InputKind::Rules(Box::new(support::rules_with(&defaults, WORKFLOW))));
    assert!(app.factory.holds_wake_lock());
}

#[test]
fn a_reached_budget_pauses_before_the_next_turn_with_a_notification() {
    let mut app = app();
    app.usage = Some(UsageReport {
        dollars: Some(2.5),
        tokens: Some(1000),
        tokens_reliable: true,
    });
    app.gates.push_back(ide_factory::gates::CommandOutcome {
        exit: Some(1),
        ..ide_factory::gates::CommandOutcome::default()
    });
    let id = app.order_with("o1", |o| o.budget = Some(Budget::new(Some(2.0), None, None)));
    app.settle();
    assert_eq!(
        app.condition(&id),
        Condition::Paused(PauseReason::Budget {
            scope: BudgetScope::Order
        })
    );
    assert!(
        app.get(&id).deviations().contains(&Deviation::BudgetOvershoot),
        "the late report overshot (C64)"
    );
    assert!(app.effects.iter().any(|e| matches!(
        e,
        Effect::Notify {
            notice: Notice::BudgetSpent,
            ..
        }
    )));
    assert!(app.effects.iter().any(|e| matches!(e, Effect::AddToDay { .. })));
    app.usage = None;
    app.send(InputKind::ChangeOption(
        id.clone(),
        ide_factory::options::OptionChange::Budget(Some(Budget::new(Some(10.0), None, None))),
    ));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_path_without_reliable_figures_is_named_in_the_evidence() {
    let mut app = app();
    app.usage = Some(UsageReport {
        dollars: None,
        tokens: Some(10),
        tokens_reliable: false,
    });
    let id = app.order("o1");
    app.settle();
    assert!(
        app.get(&id)
            .evidence()
            .unwrap()
            .deviations()
            .contains(&Deviation::Unmetered)
    );
}

fn with_accounts(app: &mut App) {
    let agents = [
        (Agent::Claude, "claude-subscription"),
        (Agent::Codex, "chatgpt-subscription"),
    ]
    .into_iter()
    .map(|(agent, service)| {
        let mut status = AgentStatus::ready(&agent);
        status.account = Some("me".into());
        status.service = Some(service.into());
        (agent, status)
    })
    .collect();
    app.send(InputKind::Agents(agents));
}

fn quota(app: &App, service: &str, utilization: f64) -> QuotaObservation {
    QuotaObservation {
        key: QuotaKey {
            account: "me".into(),
            service: service.into(),
            window: "five_hour".into(),
        },
        utilization: Some(utilization),
        status: QuotaStatus::Allowed,
        window_seconds: 5 * 3600,
        resets_at: None,
        observed_at: app.wall,
    }
}

#[test]
fn a_station_does_not_start_on_a_window_at_seventy_percent() {
    let mut app = app();
    with_accounts(&mut app);
    let observation = quota(&app, "claude-subscription", 0.7);
    app.send(InputKind::Quota(observation));
    let id = app.order("o1");
    app.settle();
    let builder = app.get(&id).session(ide_factory::Role::Builder).unwrap();
    assert_eq!(builder.entry().agent, Agent::Codex);
}

#[test]
fn with_every_window_full_the_order_pauses_until_a_window_frees() {
    let mut app = app();
    with_accounts(&mut app);
    for service in ["claude-subscription", "chatgpt-subscription"] {
        let observation = quota(&app, service, 0.9);
        app.send(InputKind::Quota(observation));
    }
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::Quota));
    let observation = quota(&app, "chatgpt-subscription", 0.2);
    app.send(InputKind::Quota(observation));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_quota_spent_mid_station_hands_the_station_to_the_next_entry() {
    let (mut app, id) = {
        let mut app = app();
        with_accounts(&mut app);
        app.hold.insert("send_prompt");
        let id = app.order("o1");
        app.settle();
        (app, id)
    };
    let observation = quota(&app, "claude-subscription", 1.0);
    app.send(InputKind::Quota(observation));
    let prompt = app.take("send_prompt").unwrap();
    app.hold.clear();
    let Effect::SendPrompt { prompt: p, session, .. } = prompt else {
        unreachable!()
    };
    app.send(InputKind::Session(
        id.clone(),
        session,
        SessionEvent::TurnEnded {
            prompt: Some(p),
            outcome: TurnOutcome::Failed("rate limited".into()),
            answer: None,
        },
    ));
    app.settle();
    let builder = app.get(&id).session(ide_factory::Role::Builder).unwrap();
    assert_eq!(builder.entry().agent, Agent::Codex);
    assert!(
        app.effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { text, .. } if text.contains("take over")))
    );
    assert!(app.get(&id).switched());
    assert_eq!(app.get(&id).attempts(), 0, "the switch is no failed attempt");
}

#[test]
fn a_start_failure_passes_to_the_next_entry() {
    let mut app = app();
    app.hold.insert("start_session");
    let id = app.order("o1");
    app.settle();
    app.take("start_session").unwrap();
    app.send(InputKind::SessionFailed {
        order: id.clone(),
        role: ide_factory::Role::Builder,
        readiness: ide_factory::routing::Readiness::SignInFailed("expired".into()),
    });
    app.hold.clear();
    app.settle();
    assert_eq!(
        app.get(&id).session(ide_factory::Role::Builder).unwrap().entry().agent,
        Agent::Codex
    );
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn without_any_ready_path_the_order_waits_for_the_user() {
    let mut app = app();
    app.send(InputKind::Agents(Default::default()));
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.condition(&id), Condition::Waiting(WaitReason::NoAgent));
    let agents = [Agent::Claude]
        .into_iter()
        .map(|a| (a.clone(), AgentStatus::ready(&a)))
        .collect();
    app.send(InputKind::Agents(agents));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_window_closing_pauses_and_reopening_resumes() {
    let (mut app, id) = running();
    app.send(InputKind::Windows(false));
    assert_eq!(app.condition(&id), Condition::Paused(PauseReason::NoWindow));
    app.hold.clear();
    app.pending.clear();
    app.send(InputKind::Windows(true));
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
}

#[test]
fn a_gate_past_its_deadline_is_stopped() {
    let mut app = app();
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    assert!(matches!(app.get(&id).phase(), Phase::Gate { .. }));
    app.pass(61 * MINUTE + 1);
    tick(&mut app);
    assert_eq!(app.asked("stop_gate"), 1);
    tick(&mut app);
    assert_eq!(app.asked("stop_gate"), 1, "asked once");
}

#[test]
fn the_turn_limit_counts_per_visit_of_the_build() {
    let mut app = App::new(
        Stage::One,
        rules(&format!("{WORKFLOW}[limits]\nturns_per_station = 1\n")),
    );
    app.gates.push_back(ide_factory::gates::CommandOutcome {
        exit: Some(1),
        ..ide_factory::gates::CommandOutcome::default()
    });
    let id = app.order("o1");
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance, "{:?}", app.condition(&id));
    assert_eq!(app.get(&id).turns_in(Station::Build), 2);
    assert_eq!(app.get(&id).review_rounds(), 1);
}

#[test]
fn the_window_rule_applies_again_when_the_build_is_entered_anew() {
    let mut app = app();
    with_accounts(&mut app);
    app.gates.push_back(ide_factory::gates::CommandOutcome {
        exit: Some(1),
        ..ide_factory::gates::CommandOutcome::default()
    });
    app.hold.insert("run_gate");
    let id = app.order("o1");
    app.settle();
    let builder = |app: &App| {
        app.get(&id)
            .session(ide_factory::Role::Builder)
            .unwrap()
            .entry()
            .agent
            .clone()
    };
    assert_eq!(builder(&app), Agent::Claude);
    let observation = quota(&app, "claude-subscription", 0.75);
    app.send(InputKind::Quota(observation));
    app.hold.clear();
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert_eq!(
        builder(&app),
        Agent::Codex,
        "the rework is a new visit, which begins under the 70 % rule (R11)"
    );
}

#[test]
fn a_new_day_lifts_yesterday_s_day_budget_without_any_spending() {
    let mut app = App::new(Stage::One, rules(&format!("{WORKFLOW}[budget.day]\ndollars = 1.0\n")));
    let mut day = ide_factory::budget::DailyAccount::new("2026-11-16");
    day.add("2026-11-16", None, &ide_factory::budget::Spend::new(1.0, 0, 0.0));
    let midnight = app.wall + 10 * SECOND;
    app.send(InputKind::Day(day.until(midnight)));
    let id = app.order("o1");
    app.settle();
    assert_eq!(
        app.condition(&id),
        Condition::Paused(PauseReason::Budget {
            scope: BudgetScope::Day
        })
    );
    assert!(app.factory.waits_for_day());
    let fleet = |app: &App| {
        let now = app.at();
        ide_factory::panel::Fleet::new(&app.factory, &Default::default(), now, now.wall, 0)
    };
    assert!(fleet(&app).day_spent().dollars() > 0.0);
    app.pass(20 * SECOND);
    assert_eq!(
        fleet(&app).day_spent().dollars(),
        0.0,
        "the line shows the new day before any input"
    );
    tick(&mut app);
    app.settle();
    assert_eq!(app.station(&id), Station::Acceptance);
    assert!(!app.factory.waits_for_day());
}
