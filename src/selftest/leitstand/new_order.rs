//! The end of the `leitstand` scenario (E185, E176): the sheet "Neuer Auftrag" keeps "Starten" off while the title,
//! goal or a readable budget is missing and says why, folds its options open and shut, offers no specification
//! without approval where the project does not allow it, and goes on "Abbrechen". Taken from an issue, it starts with
//! the issue's title and text, says that the text is the issue's and offers its open tasks ticked, each of which can
//! be left out.

use ide_factory::intake::{IssueOrigin, task_list};
use ide_factory::options::OptionProblem;
use ide_ui_factory::model::sheet::Prefill;
use ide_ui_factory::model::words;
use ide_ui_factory::selftest::{SheetControl, SheetSeen, SheetTyped};
use objc2::rc::Retained;
use splat_i18n::t;

use super::super::helpers::wait_until;
use super::super::{check, finish, give_up, snapshot};
use super::SCENARIO;
use crate::window::WindowController;

const ISSUE: &str = "Login fails after an update.\n\n- [ ] reproduce it\n- [ ] fix the session check\n";

/// Opens the sheet as "Neuer Auftrag …" does and waits for it.
pub(super) fn run(controller: &Retained<WindowController>) {
    ide_ui_factory::new_order(controller.handle(), controller.context().factory_ui());
    let probe = controller.clone();
    let controller = controller.clone();
    wait_until(
        move || ide_ui_factory::selftest::order_sheet(probe.handle()).is_some(),
        move |open| {
            if !open {
                return give_up(SCENARIO, "the sheet Neuer Auftrag opens");
            }
            typed(&controller);
        },
    );
}

fn seen(controller: &WindowController) -> Option<SheetSeen> {
    ide_ui_factory::selftest::order_sheet(controller.handle())
}

fn typing(title: &str, budget: &str) -> SheetTyped {
    SheetTyped {
        title: title.into(),
        goal: "Export the table as CSV.".into(),
        criteria: "- the export holds every row\n".into(),
        scope: "src/export/**".into(),
        budget: budget.into(),
    }
}

/// An empty sheet, then one whose budget is no amount, then one that can start.
fn typed(controller: &Retained<WindowController>) {
    let window = controller.handle();
    let empty = seen(controller);
    let no_title = words::problem(&OptionProblem::MissingTitle);
    check(
        "the empty sheet keeps \"Starten\" off and says that the title is missing, its options folded away",
        empty
            .as_ref()
            .is_some_and(|s| !s.start && !s.options && s.messages.contains(&no_title)),
        format!("{empty:?}"),
    );
    ide_ui_factory::selftest::type_into_order_sheet(window, &typing("CSV-Export", "zwei"));
    let unreadable = seen(controller);
    let no_amount = t("Das Budget ist kein Betrag in Dollar.").to_string();
    check(
        "a budget that is no amount keeps \"Starten\" off and says so, the title no longer missing",
        unreadable
            .as_ref()
            .is_some_and(|s| !s.start && s.messages.contains(&no_amount) && !s.messages.contains(&no_title)),
        format!("{unreadable:?}"),
    );
    ide_ui_factory::selftest::type_into_order_sheet(window, &typing("CSV-Export", "2,50"));
    let ready = seen(controller);
    check(
        "with title, goal, criteria and a budget in dollars \"Starten\" can start the order",
        ready
            .as_ref()
            .is_some_and(|s| s.start && s.title == "CSV-Export" && !s.messages.contains(&no_amount)),
        format!("{ready:?}"),
    );
    options(controller);
}

/// The options fold open with the risk choice and the box without approval, and shut again; then "Abbrechen".
fn options(controller: &Retained<WindowController>) {
    let window = controller.handle();
    let pressed = ide_ui_factory::selftest::press_order_sheet(window, SheetControl::Options)
        && ide_ui_factory::selftest::press_order_sheet(window, SheetControl::Risk(3));
    let open = seen(controller);
    let only_low = t("Ohne Freigabe nur bei Risikoklasse niedrig und nur, wenn das Projekt es erlaubt.");
    check(
        "the disclosure opens the options; without the project's leave a specification needs its approval",
        pressed
            && open
                .as_ref()
                .is_some_and(|s| s.options && s.start && !s.skip && s.labels.iter().any(|l| l == only_low)),
        format!("{open:?}"),
    );
    if let Some(sheet) = controller.window().and_then(|w| w.attachedSheet()) {
        sheet.displayIfNeeded();
        snapshot(&sheet, "leitstand-new-order");
    }
    ide_ui_factory::selftest::press_order_sheet(window, SheetControl::Options);
    let folded = seen(controller);
    check(
        "the disclosure folds the options away again",
        folded.as_ref().is_some_and(|s| !s.options),
        format!("{folded:?}"),
    );
    cancel(controller, from_issue);
}

/// Presses "Abbrechen" and waits until the sheet is gone, then goes on with `then`.
fn cancel(controller: &Retained<WindowController>, then: fn(&Retained<WindowController>)) {
    let pressed = ide_ui_factory::selftest::press_order_sheet(controller.handle(), SheetControl::Cancel);
    let probe = controller.clone();
    let controller = controller.clone();
    wait_until(
        move || probe.window().is_some_and(|w| w.attachedSheet().is_none()),
        move |gone| {
            if !pressed || !gone {
                return give_up(SCENARIO, "\"Abbrechen\" closes the sheet Neuer Auftrag");
            }
            then(&controller);
        },
    );
}

/// The sheet filled in from an issue.
fn from_issue(controller: &Retained<WindowController>) {
    let origin = IssueOrigin::freeze("https://example.invalid/issues/42", 42, Some("octo"), ISSUE);
    let prefill = Prefill::new(" Login fails ", origin);
    if !ide_ui_factory::new_order_from_issue(controller.handle(), controller.context().factory_ui(), prefill) {
        return give_up(SCENARIO, "the folder has a Leitstand for an order from an issue");
    }
    let probe = controller.clone();
    let controller = controller.clone();
    wait_until(
        move || ide_ui_factory::selftest::order_sheet(probe.handle()).is_some(),
        move |open| {
            if !open {
                return give_up(SCENARIO, "the sheet Neuer Auftrag opens for the issue");
            }
            issue_sheet(&controller);
        },
    );
}

fn issue_sheet(controller: &Retained<WindowController>) {
    let shown = seen(controller);
    let tasks: Vec<(String, bool)> = task_list(ISSUE).into_iter().map(|task| (task, true)).collect();
    check(
        "an issue fills in its title and text, says the text is the issue's and offers its open tasks ticked",
        shown.as_ref().is_some_and(|s| {
            s.title == "Login fails"
                && s.goal == ISSUE.trim()
                && s.labels.iter().any(|l| l.contains("#42"))
                && tasks.len() == 2
                && s.suggestions == tasks
                && !s.skip
        }),
        format!("{shown:?}"),
    );
    let pressed = ide_ui_factory::selftest::press_order_sheet(controller.handle(), SheetControl::Suggestion(0));
    let left_out = seen(controller);
    check(
        "a click leaves the issue's first task out and keeps the second",
        pressed
            && left_out
                .as_ref()
                .is_some_and(|s| s.suggestions.iter().map(|(_, on)| *on).eq([false, true]) && s.start),
        format!("{left_out:?}"),
    );
    cancel(controller, |_| finish(SCENARIO));
}
