//! The popover that takes a new comment on a plan, or a changed one.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSColor, NSLineBreakMode, NSPopover, NSPopoverBehavior, NSTextField, NSView, NSViewController};
use objc2_foundation::{NSRect, NSRectEdge};
use splat_i18n::{t, tf};

use super::super::tab::AgentTab;
use super::super::{style, ui};
use super::entry_tag;
use ide_ui_common::util::{ns, set_action};

const WIDTH: f64 = 300.0;

/// What the user wrote into the popover.
pub struct Asked {
    /// The comment changed, or `None` for a new one.
    pub note: Option<u64>,
    pub location: usize,
    pub quote: String,
    pub comment: String,
}

struct Ask {
    agent: u64,
    entry: u64,
    note: Option<u64>,
    location: usize,
    quote: String,
    popover: Retained<NSPopover>,
    field: Retained<NSTextField>,
}

/// The question about a plan on view.
#[derive(Default)]
pub struct PlanAsk {
    ask: RefCell<Option<Ask>>,
}

fn with_ask<R>(f: impl FnOnce(&RefCell<Option<Ask>>) -> R) -> R {
    f(&crate::host::state().plan.ask.ask)
}

/// Shows the popover at `rect` of `anchor` for comment `note` of plan `entry`, or a new one, on the passage `quote`
/// at `location`; the field starts with `current`.
pub fn open(
    tab: &AgentTab,
    entry: u64,
    note: Option<u64>,
    (location, quote): (usize, String),
    current: &str,
    anchor: &NSView,
    rect: NSRect,
) {
    close();
    let mtm = tab.mtm;
    let heading = if quote.trim().is_empty() {
        t("Kommentar zum ganzen Plan").to_string()
    } else {
        let shown: String = quote.trim().chars().take(80).collect();
        tf("Kommentar zu „{}“", &[&shown.replace('\n', " ")])
    };
    let heading = ui::wrapping(mtm, &heading, &style::small(), &NSColor::secondaryLabelColor(), WIDTH);
    heading.setMaximumNumberOfLines(3);
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setPlaceholderString(Some(&ns(t("Was soll anders werden?"))));
    field.setFont(Some(&style::body()));
    if let Some(cell) = field.cell() {
        cell.setWraps(true);
        cell.setScrollable(false);
        cell.setUsesSingleLineMode(false);
    }
    field.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    ui::fixed_width(&field, WIDTH);
    ui::fixed_height(&field, 58.0);
    let tag = entry_tag(entry, 0);
    set_action(&field, &tab.target, sel!(ideAgentPlanCommit:));
    field.setTag(tag);
    let cancel = ui::button(mtm, t("Abbrechen"), &tab.target, sel!(ideAgentPlanCancel:), tag);
    cancel.setKeyEquivalent(&ns("\u{1b}"));
    let title = if note.is_some() { t("Sichern") } else { t("Hinzufügen") };
    let add = ui::button(mtm, title, &tab.target, sel!(ideAgentPlanCommit:), tag);
    add.setKeyEquivalent(&ns("\r"));
    let spacer = ui::spacer(mtm);
    let buttons = ui::row(mtm, 8.0, &[&spacer, &cancel, &add]);
    let column = ui::column(mtm, 8.0, &[&heading, &field, &buttons]);
    column.setEdgeInsets(ui::insets(12.0, 12.0, 12.0, 12.0));
    let controller = NSViewController::new(mtm);
    controller.setView(&column);
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setContentViewController(Some(&controller));
    popover.showRelativeToRect_ofView_preferredEdge(rect, anchor, NSRectEdge::MaxY);
    if let Some(window) = field.window() {
        window.makeFirstResponder(Some(&field));
    }
    let ask = Ask {
        agent: tab.key,
        entry,
        note,
        location,
        quote,
        popover,
        field,
    };
    with_ask(|a| *a.borrow_mut() = Some(ask));
}

/// Closes the popover of plan `entry` in the AI tab `agent` and hands over what it holds.
pub fn take(agent: u64, entry: u64) -> Option<Asked> {
    let ask = with_ask(|a| {
        let mut a = a.borrow_mut();
        if a.as_ref().is_some_and(|ask| ask.agent == agent && ask.entry == entry) {
            a.take()
        } else {
            None
        }
    })?;
    ask.popover.close();
    Some(Asked {
        note: ask.note,
        location: ask.location,
        quote: ask.quote,
        comment: ask.field.stringValue().to_string(),
    })
}

pub fn close() {
    if let Some(ask) = with_ask(|a| a.borrow_mut().take()) {
        ask.popover.close();
    }
}

/// Types `text` into the open popover, for the self-test; whether one is open.
#[cfg(feature = "selftest")]
pub fn type_comment(text: &str) -> bool {
    with_ask(|a| {
        a.borrow()
            .as_ref()
            .map(|ask| ask.field.setStringValue(&ns(text)))
            .is_some()
    })
}

/// The open popover's content and its field's text, for the self-test.
#[cfg(feature = "selftest")]
pub fn view() -> Option<(Retained<NSView>, String)> {
    with_ask(|a| {
        let a = a.borrow();
        let ask = a.as_ref()?;
        let view = ask.popover.contentViewController()?.view();
        Some((view, ask.field.stringValue().to_string()))
    })
}
