//! The body of an order's tab: what waits for the user as cards, how far its merge request got (E177), goal and
//! criteria, the timeline, the evidence package with its deviations first (R70), and the sections of the diff with a
//! checkbox each for G2 (C55). Logs and the diff's places stay left to right (E3).

use std::collections::BTreeSet;

use ide_factory::HunkId;
use ide_ui_common::style::space;
use ide_ui_common::util::{ns, symbol};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSBox, NSButton, NSColor, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation,
    NSStackView, NSTextField, NSView,
};
use splat_i18n::{t, tf};

use crate::model::detail::{Detail, Passage};
use crate::order::OrderTab;
use crate::ui;

/// How wide text wraps until the tab wraps it at its own width.
const WIDTH: f64 = 640.0;

/// Fills `body` with what `detail` shows; `rejected` are the sections turned down so far. Gives the timeline's
/// labels, in the order of [`timeline_texts`].
pub(crate) fn fill(
    tab: &OrderTab,
    body: &NSStackView,
    detail: &Detail,
    rejected: &BTreeSet<HunkId>,
) -> Vec<Retained<NSTextField>> {
    let mtm = tab.mtm();
    if let Some(objection) = detail.objection() {
        let reasons = if objection.is_empty() {
            t("Der Builder hat keinen Grund genannt.")
        } else {
            objection
        };
        let explained = ui::note(
            mtm,
            t(
                "„Kriterien ändern …“ passt die Kriterien an, „Weiter“ lässt sie gelten, wie sie sind, und „Auftrag abbrechen …“ beendet den Auftrag.",
            ),
            WIDTH,
        );
        card(
            mtm,
            body,
            t("Einwand des Builders"),
            &[&ui::text(mtm, reasons, WIDTH), &explained],
        );
    }
    if let Some(change) = detail.issue_change() {
        issue_change(tab, body, detail, change);
    }
    if !detail.publication().is_empty() {
        section(mtm, body, t("Merge Request"));
        lines(mtm, body, detail.publication(), false);
        divide(mtm, body);
    }
    section(mtm, body, t("Ziel"));
    body.addArrangedSubview(&ui::text(mtm, detail.goal(), WIDTH));
    section(mtm, body, t("Kriterien"));
    lines(mtm, body, detail.criteria(), false);
    divide(mtm, body);
    let ages = timeline(mtm, body, detail);
    divide(mtm, body);
    evidence(mtm, body, detail);
    if !detail.sections().is_empty() {
        divide(mtm, body);
        sections(tab, body, detail, rejected);
    }
    ages
}

/// The text of each station's row on the timeline, then the line of those still ahead, if any.
pub(crate) fn timeline_texts(detail: &Detail) -> Vec<String> {
    let (ahead, rows): (Vec<_>, Vec<_>) = detail
        .timeline()
        .iter()
        .partition(|stage| stage.passage() == Passage::Ahead);
    let mut texts: Vec<String> = rows.iter().map(|stage| stage.words()).collect();
    if !ahead.is_empty() {
        let ahead: Vec<String> = ahead.iter().map(|stage| stage.words()).collect();
        texts.push(tf("Noch offen: {}", &[&ahead.join(" · ")]));
    }
    texts
}

/// The stations the order passed, stands at or skipped, a row each, and those still ahead on one line; gives the
/// labels of their texts.
fn timeline(mtm: MainThreadMarker, body: &NSStackView, detail: &Detail) -> Vec<Retained<NSTextField>> {
    section(mtm, body, t("Zeitleiste"));
    let mut texts = timeline_texts(detail).into_iter();
    let mut labels = Vec::new();
    for stage in detail
        .timeline()
        .iter()
        .filter(|stage| stage.passage() != Passage::Ahead)
    {
        let passage = passage_words(stage.passage());
        let tint = if stage.passage() == Passage::Current {
            NSColor::controlAccentColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        let label = ui::text(mtm, &texts.next().unwrap_or_default(), WIDTH);
        body.addArrangedSubview(&marked(mtm, stage.symbol(), passage, &tint, &label));
        labels.push(label);
    }
    if let Some(ahead) = texts.next() {
        let label = ui::note(mtm, &ahead, WIDTH);
        body.addArrangedSubview(&label);
        labels.push(label);
    }
    labels
}

/// A row of the symbol `name`, named `meaning` for VoiceOver and tinted `tint`, before `words`.
fn marked(
    mtm: MainThreadMarker,
    name: &str,
    meaning: &str,
    tint: &NSColor,
    words: &NSTextField,
) -> Retained<NSStackView> {
    let row = ui::row(mtm, space::S);
    row.setAlignment(NSLayoutAttribute::Top);
    let image = NSImageView::new(mtm);
    image.setImage(symbol(name, meaning).as_deref());
    image.setContentTintColor(Some(tint));
    image.setAccessibilityLabel(Some(&ns(meaning)));
    image.setContentHuggingPriority_forOrientation(751.0, NSLayoutConstraintOrientation::Horizontal);
    row.addArrangedSubview(&image);
    row.addArrangedSubview(words);
    row
}

/// A box of `views` under `title` across the whole width, for what waits for the user.
fn card(mtm: MainThreadMarker, body: &NSStackView, title: &str, views: &[&NSView]) {
    let card = NSBox::new(mtm);
    card.setTitle(&ns(title));
    let column = ui::column(mtm, space::S);
    for view in views {
        column.addArrangedSubview(view);
    }
    card.setContentView(Some(&column));
    body.addArrangedSubview(&card);
    card.widthAnchor()
        .constraintEqualToAnchor(&body.widthAnchor())
        .setActive(true);
    body.setCustomSpacing_afterView(space::SECTION, &card);
}

/// A line across `body` between two sections.
fn divide(mtm: MainThreadMarker, body: &NSStackView) {
    if let Some(last) = body.arrangedSubviews().lastObject() {
        body.setCustomSpacing_afterView(space::SECTION, &last);
    }
    let line = ui::rule(mtm);
    body.addArrangedSubview(&line);
    line.widthAnchor()
        .constraintEqualToAnchor(&body.widthAnchor())
        .setActive(true);
    body.setCustomSpacing_afterView(space::SECTION, &line);
}

/// The issue's change since the order came in, with "Änderung übernehmen" (E176).
fn issue_change(tab: &OrderTab, body: &NSStackView, detail: &Detail, change: &[String]) {
    let mtm = tab.mtm();
    let explained = ui::note(
        mtm,
        t(
            "Das Issue liest sich anders als beim Eingang des Auftrags. Erst „Änderung übernehmen“ ändert Ziel und Kriterien; ein gültiger Nachweis wird dabei ungültig.",
        ),
        WIDTH,
    );
    let take = ui::button(mtm, t("Änderung übernehmen"), tab.me(), sel!(ideFactoryTakeIssue:), 0);
    take.setEnabled(detail.take_issue_change().is_some());
    card(
        mtm,
        body,
        t("Issue geändert"),
        &[&explained, &ui::code(mtm, &change.join("\n"), WIDTH), &take],
    );
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
    body.addArrangedSubview(&ui::subheading(mtm, t("Abweichungen")));
    if detail.deviations().is_empty() {
        body.addArrangedSubview(&ui::note(mtm, t("keine"), WIDTH));
    }
    let warning = NSColor::systemOrangeColor();
    for deviation in detail.deviations() {
        let row = marked(
            mtm,
            "exclamationmark.triangle.fill",
            t("Abweichung"),
            &warning,
            &ui::text(mtm, deviation, WIDTH),
        );
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
    let mut nothing = Vec::new();
    for (title, items, code) in [
        (t("Gates"), detail.gates(), true),
        (t("Prüfrunden"), detail.rounds(), false),
        (t("Kosten und Laufzeit"), detail.costs(), false),
        (t("Herkunft"), detail.provenance(), true),
    ] {
        if items.is_empty() {
            nothing.push(title);
            continue;
        }
        body.addArrangedSubview(&ui::subheading(mtm, title));
        lines(mtm, body, items, code);
    }
    if detail.sections().is_empty() {
        nothing.push(t("Abschnitte des Diffs"));
    }
    if !nothing.is_empty() {
        body.addArrangedSubview(&ui::note(mtm, &tf("Noch nichts: {}", &[&nothing.join(", ")]), WIDTH));
    }
}

fn sections(tab: &OrderTab, body: &NSStackView, detail: &Detail, rejected: &BTreeSet<HunkId>) {
    let mtm = tab.mtm();
    section(mtm, body, t("Abschnitte des Diffs"));
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
    button.setToolTip(Some(&ns(title)));
    button.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingMiddle);
    button.setContentCompressionResistancePriority_forOrientation(
        ui::GIVES_WAY,
        NSLayoutConstraintOrientation::Horizontal,
    );
    button.setFont(Some(&ide_ui_common::style::mono(ide_ui_common::style::CODE)));
    button.setUserInterfaceLayoutDirection(objc2_app_kit::NSUserInterfaceLayoutDirection::LeftToRight);
    button
}
