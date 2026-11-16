//! The `leitstand` scenario (E175): a folder without orders shows the Leitstand panel's empty state under ⌘9, its
//! symbol named only "Leitstand" since no order waits (E69); the Leitstand tab opens with every station of the strip
//! at zero, the table's six columns and its empty note; window pictures of both and the accessibility audit. Its
//! children are the scenarios that run orders.

pub(super) mod attach;
pub(super) mod metrics;
pub(super) mod order;
pub(super) mod queue;

use objc2::rc::Retained;

use super::helpers::{self, wait_until};
use super::{check, finish, give_up, snapshot};
use crate::window::{Panel, WindowController};

const SCENARIO: &str = "leitstand";
const PROJECT: &str = "leitstand-project";

pub(crate) fn run() {
    helpers::open_project(SCENARIO, PROJECT, &[("README.md", "# Leitstand\n")], |controller| {
        controller.activate_panel(Panel::Leitstand);
        let probe = controller.clone();
        wait_until(
            move || ide_ui_factory::selftest::panel(probe.handle(), probe.context().factory_ui()).is_some(),
            move |found| {
                if !found {
                    return give_up(SCENARIO, "the Leitstand panel is in place");
                }
                empty_panel(&controller);
            },
        );
    });
}

/// The panel without orders: its empty state, no station, and the symbol's words without a waiting order.
fn empty_panel(controller: &Retained<WindowController>) {
    let seen = ide_ui_factory::selftest::panel(controller.handle(), controller.context().factory_ui());
    check(
        "a folder without orders shows the Leitstand panel's empty state and no station",
        seen.as_ref().is_some_and(|seen| seen.empty && seen.lines.is_empty()),
        format!("{seen:?}"),
    );
    let (tip, _) = controller.symbol_tip(Panel::Leitstand);
    check(
        "the symbol names the Leitstand alone while no order waits",
        tip.as_deref() == Some(Panel::Leitstand.title()),
        format!("{tip:?}"),
    );
    if let Some(window) = controller.window() {
        window.displayIfNeeded();
        snapshot(&window, "leitstand-panel");
    }
    let shown = ide_ui_factory::show_leitstand(controller.handle(), controller.context().factory_ui());
    let probe = controller.clone();
    let controller = controller.clone();
    wait_until(
        move || ide_ui_factory::selftest::board(probe.handle(), probe.context().factory_ui()).is_some(),
        move |found| {
            if !shown || !found {
                return give_up(SCENARIO, "the Leitstand tab opens");
            }
            empty_tab(&controller);
        },
    );
}

/// The tab without orders: every station at zero, the six columns, the empty note, and nothing the audit finds.
fn empty_tab(controller: &Retained<WindowController>) {
    let seen = ide_ui_factory::selftest::board(controller.handle(), controller.context().factory_ui());
    let empty = ide_ui_factory::model::words::no_order_yet();
    check(
        "the Leitstand tab shows seven stations at zero, six columns, no row and its empty note",
        seen.as_ref().is_some_and(|seen| {
            seen.strip.len() == 7
                && seen.strip.iter().all(|stop| stop.ends_with(" 0"))
                && seen.columns.len() == 6
                && seen.rows == 0
                && seen.empty == empty
        }),
        format!("{seen:?}"),
    );
    let Some(window) = controller.window() else {
        return give_up(SCENARIO, "the window is there");
    };
    window.displayIfNeeded();
    snapshot(&window, "leitstand-tab");
    let findings = super::a11y::audit::findings(&super::a11y::audit::elements(&window));
    check(
        "the accessibility audit finds nothing in the window with the Leitstand panel and tab",
        findings.is_empty(),
        findings
            .iter()
            .map(|f| format!("{}: {} ({})", f.problem, f.what, f.path))
            .collect::<Vec<_>>()
            .join("; "),
    );
    finish(SCENARIO);
}
