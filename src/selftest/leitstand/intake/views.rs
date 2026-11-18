//! The views' host answering as the runtime does (B42): the sheet's context, refusals of what the runtime does not take
//! now, the project file and the user's defaults, a new order from the sheet, and what the order tab opens in the
//! window: a terminal, the merge request's pages, the worktree and the tabs that come back with the layout. At the end
//! both orders are aborted, which ends their setup commands.

use std::path::Path;
use std::rc::Rc;

use ide_factory::options::NewOrder;
use ide_factory::workflow::WORKFLOW_FILE;
use ide_factory::{Condition, InputKind, OrderId, Refusal, Refused, RequestId};
use ide_process::Supervisor;
use ide_ui_factory::Host;

use super::super::super::helpers::{wait_within, window_of};
use super::super::super::{check, finish, give_up};
use super::{Flow, READ, SCENARIO, SHORT, WORKFLOW, host};
use crate::leitstand::Command;
use crate::window::{Panel, WindowController};

pub(super) fn run(flow: Flow, id: OrderId) {
    let cx = crate::context::app();
    let host = host();
    let root = &flow.root;
    check(
        "the views' sheet checks against what the runtime knows of the folder",
        format!("{:?}", host.sheet(root)) == format!("{:?}", cx.leitstand().sheet(root)),
        "",
    );
    let not_now = |order: Option<OrderId>| {
        vec![Refusal {
            order,
            why: Refused::NotNow,
        }]
    };
    let tick = host.send(root, InputKind::Tick);
    let resumed = host.send(root, InputKind::Continue(id.clone()));
    check(
        "what is no user command is refused as not now, and so is Weiter on an order that does not wait",
        tick == not_now(None) && resumed == not_now(Some(id.clone())),
        format!("{tick:?}, {resumed:?}"),
    );
    let before = cx.leitstand().refusals(root);
    let decided = host.decide(root, &id, &RequestId("selftest-none".into()), true);
    check(
        "a decision on a request no session asked for changes nothing",
        decided == before,
        format!("{decided:?}"),
    );
    project_file(&host, root);
    let new = host.new_order(
        root,
        NewOrder {
            title: "Zweiter Auftrag".into(),
            goal: "Nothing to change.".into(),
            criteria: "- nothing changes\n".into(),
            ..NewOrder::default()
        },
    );
    let second = new.as_ref().ok().cloned();
    // The runtime reads the folder's branch on the git lane before the order comes in.
    let (probe, root) = (second.clone(), root.to_path_buf());
    wait_within(
        SHORT,
        move || {
            probe
                .as_ref()
                .is_some_and(|second| crate::context::app().leitstand().order_ids(&root).contains(second))
        },
        move |came| {
            check("an order from the views' sheet comes in", came, format!("{new:?}"));
            window_parts(flow, id, second);
        },
    );
}

/// The project file the views edit is the folder's workflow file, saved as it is it stays, and so do the user's
/// defaults saved unchanged.
fn project_file(host: &crate::leitstand_ui::Orders, root: &Path) {
    let file = host.project_file(root);
    let saved = host.save_project_file(root, file.text());
    let on_disk = std::fs::read_to_string(root.join(WORKFLOW_FILE)).unwrap_or_default();
    check(
        "the views edit the folder's workflow file, and saving it unchanged writes the same text",
        file.text() == WORKFLOW && saved.is_ok() && on_disk == WORKFLOW,
        format!("{:?}, saved {}", file.text(), saved.is_ok()),
    );
    let defaults = host.defaults();
    host.set_defaults(defaults.clone());
    check(
        "the user's defaults saved unchanged read back the same",
        host.defaults() == defaults,
        "",
    );
}

/// The tab selected after `open` ran, closed again, if `open` opened one.
fn opened_tab(controller: &WindowController, open: impl FnOnce()) -> Option<crate::window::TabId> {
    let before = controller.tab_ids();
    open();
    controller.selected_tab().filter(|tab| !before.contains(tab))
}

/// What the order tab opens in the window: a terminal, the pages of a merge request, the worktree in the Git panel,
/// and the tabs that come back with the layout.
fn window_parts(flow: Flow, id: OrderId, second: Option<OrderId>) {
    let Some(controller) = window_of(&flow.root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let host = host();
    let window = controller.id();
    let terminal = opened_tab(&controller, || host.open_terminal(window, &flow.root));
    if let Some(tab) = terminal {
        controller.close_tab(tab);
    }
    check(
        "the views open a terminal as a tab of the window",
        terminal.is_some(),
        "",
    );
    let page = format!("http://127.0.0.1:{}/acme/widget/pull/32", flow.stub.port);
    let change = opened_tab(&controller, || host.open_change(window, &page));
    let shown = change.and_then(|tab| crate::browser::url_of_tab(&controller, tab));
    if let Some(tab) = change {
        controller.close_tab(tab);
    }
    check(
        "the views open a merge request's page in a browser tab of the window",
        change.is_some() && shown.as_deref().is_none_or(|shown| shown == page),
        format!("{shown:?}"),
    );
    let before = controller.tab_ids();
    host.open_merge_request(window, &flow.root, "order/o-1");
    let probe = (flow.root.clone(), before);
    wait_within(
        READ,
        move || new_change_tab(&probe.0, &probe.1).is_some(),
        move |_| {
            let found = new_change_tab(&flow.root, &[]);
            if let (Some(controller), Some((tab, _))) = (window_of(&flow.root), found.as_ref()) {
                controller.close_tab(*tab);
            }
            check(
                "the views open the page of a new merge request of the order's branch on the folder's service",
                found.is_some(),
                "",
            );
            restored_tabs(flow, id, second);
        },
    );
}

/// A browser tab of the folder's window not among `before` that shows the page of a new merge request of the
/// order's branch, with its address.
fn new_change_tab(root: &Path, before: &[crate::window::TabId]) -> Option<(crate::window::TabId, String)> {
    let controller = window_of(root)?;
    controller
        .tab_ids()
        .into_iter()
        .filter(|tab| !before.contains(tab))
        .find_map(|tab| {
            let url = crate::browser::url_of_tab(&controller, tab)?;
            url.contains("/acme/widget/compare/order%2Fo-1").then_some((tab, url))
        })
}

/// An order without a session or specification shows its worktree; the Leitstand tab and the order's tab come back
/// with the layout.
fn restored_tabs(flow: Flow, id: OrderId, second: Option<OrderId>) {
    let Some(controller) = window_of(&flow.root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let host = host();
    let window = controller.id();
    let issue = host.spec_view(window, &flow.root, &id, Rc::new(|_| {}));
    let manual = second
        .as_ref()
        .and_then(|second| host.spec_view(window, &flow.root, second, Rc::new(|_| {})));
    check(
        "the order from an issue has a view of its specification, the one from the sheet has none",
        issue.is_some() && second.is_some() && manual.is_none(),
        format!("{} {}", issue.is_some(), manual.is_some()),
    );
    host.open_session(window, &flow.root, &id);
    let worktree = crate::context::app().leitstand().worktree(&flow.root, &id);
    let panel = controller.inspect().panel;
    check(
        "an order without a session shows its worktree in the Git panel once it has one",
        worktree.is_none() || panel == Some(Panel::Git),
        format!("{worktree:?}, {panel:?}"),
    );
    let board = crate::leitstand_ui::reopen_leitstand(&controller);
    let order = crate::leitstand_ui::reopen_order(&controller, &id.0);
    check(
        "the Leitstand tab and the order's tab open again as the layout brings them back",
        board && order,
        format!("{board} {order}"),
    );
    abort(flow, [Some(id), second].into_iter().flatten().collect());
}

/// Whether the setup command of `id` runs now, as the supervisor knows it.
fn setup_runs(id: &OrderId) -> bool {
    let label = splat_i18n::tf("Einrichtung von „{}“", &[&id.0]);
    Supervisor::global().running().iter().any(|p| p.label == label)
}

/// Aborts `orders`, which ends their setup commands, and ends the scenario.
fn abort(flow: Flow, orders: Vec<OrderId>) {
    let cx = crate::context::app();
    for id in &orders {
        cx.leitstand().command(&cx, &flow.root, Command::Abort(id.clone()));
    }
    let probe = (flow.root.clone(), orders.clone());
    wait_within(
        SHORT,
        move || {
            probe.1.iter().all(|id| {
                !setup_runs(id)
                    && crate::context::app()
                        .leitstand()
                        .with_order(&probe.0, id, |o| *o.condition() == Condition::Aborted)
                        .unwrap_or(false)
            })
        },
        move |ended| {
            check(
                "aborting the orders ends their setup commands",
                ended && orders.len() == 2,
                format!("{orders:?}"),
            );
            let cx = crate::context::app();
            cx.leitstand().use_script(None);
            cx.leitstand().use_agents(None);
            finish(SCENARIO);
        },
    );
}
