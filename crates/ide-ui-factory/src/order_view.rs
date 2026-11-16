//! The body of an order's tab: goal and criteria, the timeline, the evidence package with its deviations first (R70),
//! and the sections of the diff with a checkbox each for G2 (C55). Logs and the diff's places stay left to right (E3).

use std::collections::BTreeSet;

use ide_factory::HunkId;
use ide_ui_common::style::space;
use ide_ui_common::util::{ns, symbol};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAccessibility, NSButton, NSColor, NSImageView, NSStackView};
use splat_i18n::t;

use crate::model::detail::{Detail, Passage};
use crate::order::OrderTab;
use crate::ui;

const WIDTH: f64 = 640.0;

/// Fills `body` with what `detail` shows; `rejected` are the sections turned down so far.
pub(crate) fn fill(tab: &OrderTab, body: &NSStackView, detail: &Detail, rejected: &BTreeSet<HunkId>) {
    let mtm = tab.mtm();
    section(mtm, body, t("Ziel"));
    body.addArrangedSubview(&ui::text(mtm, detail.goal(), WIDTH));
    section(mtm, body, t("Kriterien"));
    lines(mtm, body, detail.criteria(), false);
    section(mtm, body, t("Zeitleiste"));
    for stage in detail.timeline() {
        let row = ui::row(mtm, space::S);
        let image = NSImageView::new(mtm);
        let passage = passage_words(stage.passage());
        image.setImage(symbol(stage.symbol(), passage).as_deref());
        let tint = if stage.passage() == Passage::Current {
            NSColor::controlAccentColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        image.setContentTintColor(Some(&tint));
        image.setAccessibilityLabel(Some(&ns(passage)));
        row.addArrangedSubview(&image);
        row.addArrangedSubview(&ui::text(mtm, stage.words(), WIDTH));
        body.addArrangedSubview(&row);
    }
    evidence(mtm, body, detail);
    sections(tab, body, detail, rejected);
}

fn passage_words(passage: Passage) -> &'static str {
    match passage {
        Passage::Passed => t("durchlaufen"),
        Passage::Current => t("aktuell"),
        Passage::Skipped => t("übersprungen"),
        Passage::Ahead => t("noch nicht erreicht"),
    }
}

fn section(mtm: MainThreadMarker, body: &NSStackView, words: &str) {
    let heading = ui::heading(mtm, words);
    body.addArrangedSubview(&heading);
    body.setCustomSpacing_afterView(space::XS, &heading);
}

/// Each of `items` on a line of its own; `code` keeps them monospaced and left to right.
fn lines(mtm: MainThreadMarker, body: &NSStackView, items: &[String], code: bool) {
    if items.is_empty() {
        body.addArrangedSubview(&ui::note(mtm, t("keine"), WIDTH));
    }
    for item in items {
        let field = if code {
            ui::code(mtm, item, WIDTH)
        } else {
            ui::text(mtm, item, WIDTH)
        };
        body.addArrangedSubview(&field);
    }
}

fn evidence(mtm: MainThreadMarker, body: &NSStackView, detail: &Detail) {
    section(mtm, body, t("Nachweis"));
    let deviations = ui::heading(mtm, t("Abweichungen"));
    body.addArrangedSubview(&deviations);
    if detail.deviations().is_empty() {
        body.addArrangedSubview(&ui::note(mtm, t("keine"), WIDTH));
    }
    for deviation in detail.deviations() {
        let row = ui::row(mtm, space::S);
        let image = NSImageView::new(mtm);
        image.setImage(symbol("exclamationmark.triangle.fill", t("Abweichung")).as_deref());
        image.setContentTintColor(Some(&NSColor::systemOrangeColor()));
        image.setAccessibilityLabel(Some(&ns(t("Abweichung"))));
        row.addArrangedSubview(&image);
        row.addArrangedSubview(&ui::text(mtm, deviation, WIDTH));
        body.addArrangedSubview(&row);
    }
    if detail.evidence().is_empty() {
        body.addArrangedSubview(&ui::note(
            mtm,
            t("Noch kein Nachweis: Er entsteht, wenn alle Gates bestanden haben."),
            WIDTH,
        ));
    } else {
        lines(mtm, body, detail.evidence(), false);
    }
    for (title, items, code) in [
        (t("Gates"), detail.gates(), true),
        (t("Prüfrunden"), detail.rounds(), false),
        (t("Kosten und Laufzeit"), detail.costs(), false),
        (t("Herkunft"), detail.provenance(), true),
    ] {
        body.addArrangedSubview(&ui::heading(mtm, title));
        lines(mtm, body, items, code);
    }
}

fn sections(tab: &OrderTab, body: &NSStackView, detail: &Detail, rejected: &BTreeSet<HunkId>) {
    let mtm = tab.mtm();
    section(mtm, body, t("Abschnitte des Diffs"));
    if detail.sections().is_empty() {
        body.addArrangedSubview(&ui::note(mtm, t("Der Diff hat noch keine Abschnitte."), WIDTH));
        return;
    }
    body.addArrangedSubview(&ui::note(
        mtm,
        t("Ein abgewählter Abschnitt wird bei der Abnahme verworfen; aus dem Rest entsteht ein neuer Stand, den Gates und Prüfer erneut prüfen."),
        WIDTH,
    ));
    for (index, part) in detail.sections().iter().enumerate() {
        let choice = checkbox(mtm, tab, &part.title(), index);
        choice.setState(OrderTab::state_of(rejected, part.id()));
        choice.setEnabled(detail.can(crate::model::detail::Command::Accept));
        body.addArrangedSubview(&choice);
    }
}

fn checkbox(mtm: MainThreadMarker, tab: &OrderTab, title: &str, index: usize) -> Retained<NSButton> {
    // SAFETY: The tab answers the action and lives as long as its checkboxes, which it holds.
    let button = unsafe {
        NSButton::checkboxWithTitle_target_action(&ns(title), Some(tab.me()), Some(sel!(ideFactorySection:)), mtm)
    };
    button.setTag(isize::try_from(index).unwrap_or(0));
    button.setFont(Some(&ide_ui_common::style::mono(ide_ui_common::style::CODE)));
    button.setUserInterfaceLayoutDirection(objc2_app_kit::NSUserInterfaceLayoutDirection::LeftToRight);
    button
}
