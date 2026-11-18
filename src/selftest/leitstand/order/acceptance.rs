//! The `leitstand-order` scenario at acceptance, acting on the order as the user does: its row in the panel, the menu
//! "Ablage ▸ Auftrag", and "Zurück an den Builder …", which asks first and sends nothing on "Abbrechen".

use std::path::{Path, PathBuf};

use ide_factory::{Condition, OrderId, WaitReason};
use ide_ui_common::window::Tabs;
use ide_ui_factory::model::detail::Command as OrderCommand;
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSAlertSecondButtonReturn, NSMenuItem};

use super::super::super::helpers;
use super::super::super::{check, give_up};
use super::super::drive;
use super::{REPORT, SCENARIO, TITLE, state};
use crate::window::WindowController;

/// The panel's rows show what its list holds, the waiting order with its title, its condition's symbol and the red
/// dot.
pub(super) fn row(controller: &WindowController) {
    let (window, ui) = (controller.handle(), controller.context().factory_ui());
    let rows = ide_ui_factory::selftest::panel_rows(window, ui).unwrap_or_default();
    let lines = ide_ui_factory::selftest::panel(window, ui)
        .map(|seen| seen.lines)
        .unwrap_or_default();
    let firsts: Vec<String> = rows
        .iter()
        .map(|row| row.texts.first().cloned().unwrap_or_default())
        .collect();
    check(
        "the panel's rows show its list, the waiting order with its title, its symbol and the red dot",
        !rows.is_empty()
            && firsts == lines
            && rows
                .iter()
                .any(|row| row.texts.first().map(String::as_str) == Some(TITLE) && row.symbols == 2),
        format!("{rows:?} for {lines:?}"),
    );
}

/// "Ablage ▸ Auftrag" acts on the order whose tab is in front: it can be opened and its report shown, G1 is greyed
/// out; the menu brings the Leitstand tab to the front, opens the order again and its report in an editor.
pub(super) fn menus(controller: &WindowController, id: &OrderId, tree: &Path) {
    let item = |action, command: Option<OrderCommand>| drive::menu_item(action, command.map(OrderCommand::tag));
    let open = item(sel!(ideFactoryMenuOpen:), None);
    let approve = item(sel!(ideFactoryMenuCommand:), Some(OrderCommand::Approve));
    let report = item(sel!(ideFactoryMenuCommand:), Some(OrderCommand::OpenReport));
    let board = item(sel!(ideFactoryMenuBoard:), None);
    let settings = item(sel!(ideFactoryMenuSettings:), None);
    let enabled = |item: &Option<Retained<NSMenuItem>>| item.as_deref().map(drive::enabled);
    let states = [&open, &approve, &report, &board, &settings].map(enabled);
    check(
        "with the order's tab in front, Ablage ▸ Auftrag offers to open it and its report and greys G1 out",
        states == [Some(true), Some(false), Some(true), Some(true), Some(true)],
        format!("{states:?}"),
    );
    let (Some(open), Some(report), Some(board)) = (open, report, board) else {
        return give_up(SCENARIO, "the File menu has the submenu Auftrag");
    };
    let (window, ui) = (controller.handle(), controller.context().factory_ui());
    let key = ide_ui_factory::order_tab_key(id);
    drive::choose(&board);
    let order_front = || window.selected_tab().is_some() && window.selected_tab() == window.tab_with_key(&key);
    check(
        "\"Software-Factory-Tab zeigen\" brings the Leitstand tab to the front",
        ide_ui_factory::selftest::board(window, ui).is_some() && !order_front(),
        format!("{:?}", window.selected_tab()),
    );
    ide_ui_factory::open_order(window, ui, id);
    drive::choose(&open);
    check(
        "\"Auftrag öffnen\" keeps the order in front",
        order_front(),
        format!("{:?}", window.selected_tab()),
    );
    drive::choose(&report);
    let path = tree.join(REPORT);
    check(
        "\"Bericht öffnen\" from the menu opens the report the order wrote in an editor",
        controller.tab_with_key(&path.to_string_lossy()).is_some(),
        path.display().to_string(),
    );
}

/// "Zurück an den Builder …" asks for a comment in a sheet that names the order; "Abbrechen" sends nothing and the
/// order still waits for acceptance; then `then`.
pub(super) fn back_to_builder(root: PathBuf, id: OrderId, then: fn(PathBuf, OrderId)) {
    let pressed = helpers::window_of(&root).is_some_and(|controller| {
        let (window, ui) = (controller.handle(), controller.context().factory_ui());
        ide_ui_factory::selftest::press_order(window, ui, &id, OrderCommand::BackToBuilder)
    });
    if !pressed {
        return give_up(SCENARIO, "the order's tab is open at acceptance");
    }
    drive::when_alert(root, move |root, shown| {
        let texts = drive::alert_texts(&root);
        check(
            "\"Zurück an den Builder …\" asks for a comment in a sheet that names the order",
            shown && texts.iter().any(|text| text.contains(TITLE)),
            format!("{texts:?}"),
        );
        drive::answer(root, NSAlertSecondButtonReturn, move |root, answered| {
            let waits = crate::context::app()
                .leitstand()
                .with_order(&root, &id, |o| {
                    *o.condition() == Condition::Waiting(WaitReason::Acceptance)
                })
                .unwrap_or(false);
            check(
                "\"Abbrechen\" sends no comment and the order still waits for acceptance",
                answered && waits,
                state(&root, &id),
            );
            then(root, id);
        });
    });
}
