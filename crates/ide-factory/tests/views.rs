//! The models behind the panel, the Leitstand tab and the fleet line (E175), and the figures of E180.

mod support;

use ide_factory::metrics::Metrics;
use ide_factory::panel::{Board, Column, Fleet, Panel};
use ide_factory::quota::{QuotaBook, QuotaKey, QuotaObservation, QuotaStatus};
use ide_factory::routing::Agent;
use ide_factory::{InputKind, SessionEvent, Stage, Station};
use support::{App, WORKFLOW, rules};

/// Three orders: one waiting for acceptance, one building, one in intake.
fn three() -> App {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let done = app.order("a");
    app.settle();
    assert_eq!(app.station(&done), Station::Acceptance);
    app.hold.insert("send_prompt");
    app.order("b");
    app.settle();
    app.order("c");
    app.settle();
    app
}

#[test]
fn the_panel_groups_by_station_and_shows_the_dot_while_an_order_waits() {
    let app = three();
    let panel = Panel::new(&app.factory, app.at());
    let stations: Vec<Station> = panel.groups().iter().map(|(s, _)| *s).collect();
    assert_eq!(stations, [Station::Intake, Station::Build, Station::Acceptance]);
    assert_eq!(panel.waiting(), 1);
    assert!(panel.dot());
    let building = &panel.groups()[1].1[0];
    assert_eq!(building.agent(), Some("claude"));
}

#[test]
fn the_strip_counts_per_station_and_filters_the_table() {
    let app = three();
    let mut board = Board::new(&app.factory, app.at());
    let strip = board.strip();
    assert_eq!(
        strip.iter().find(|(s, _)| *s == Station::Build).map(|(_, n)| *n),
        Some(1)
    );
    assert_eq!(board.rows().len(), 3);
    assert_eq!(board.rows()[0].title(), "Order a", "the longest wait comes first");
    board.filter(Some(Station::Build));
    assert_eq!(board.rows().len(), 1);
    board.filter(Some(Station::Build));
    assert_eq!(board.active_filter(), None, "a second click clears the filter");
    board.sort_by(Column::Title);
    let titles: Vec<&str> = board.rows().iter().map(|r| r.title()).collect();
    assert_eq!(titles, ["Order a", "Order b", "Order c"]);
    board.sort_by(Column::Title);
    assert_eq!(board.rows()[0].title(), "Order c");
}

#[test]
fn the_fleet_line_counts_runs_waits_quotas_and_the_median_wait() {
    let mut app = three();
    app.pass(60_000);
    let mut quotas = QuotaBook::default();
    quotas.observe(QuotaObservation {
        key: QuotaKey {
            account: "me".into(),
            service: "claude-subscription".into(),
            window: "five_hour".into(),
        },
        utilization: Some(0.4),
        status: QuotaStatus::Allowed,
        window_seconds: 18_000,
        resets_at: None,
        observed_at: app.wall - 1_000,
    });
    let fleet = Fleet::new(&app.factory, &quotas, app.at(), app.wall - 3_600_000, 5_000_000_000);
    assert_eq!((fleet.running(), fleet.waiting(), fleet.failed()), (1, 1, 0));
    assert_eq!(fleet.parallel(), 1);
    assert_eq!(fleet.quotas().len(), 1);
    assert!(fleet.quotas()[0].current());
    assert!(fleet.median_wait_ms().is_some_and(|ms| ms >= 60_000));
    assert_eq!(fleet.worktree_bytes(), 5_000_000_000);
}

#[test]
fn first_pass_rates_leave_out_orders_with_interventions() {
    let mut app = App::new(Stage::One, rules(WORKFLOW));
    let clean = app.order("clean");
    app.settle();
    app.hold.insert("send_prompt");
    let helped = app.order("helped");
    app.settle();
    let session = app.turn_session(&helped);
    app.send(InputKind::Session(helped.clone(), session, SessionEvent::Intervened));
    app.hold.clear();
    app.settle();
    assert_eq!(app.station(&clean), Station::Acceptance);
    assert_eq!(app.station(&helped), Station::Acceptance);
    let metrics = Metrics::new(app.factory.orders(), app.wall);
    assert_eq!(metrics.first_pass_rate(&Agent::Claude, None), Some(1.0));
    let builder = metrics
        .figures()
        .iter()
        .find(|(k, _)| k.role() == ide_factory::Role::Builder)
        .map(|(_, f)| f.orders());
    assert_eq!(builder, Some(2), "both orders count for cost and runtime");
}
