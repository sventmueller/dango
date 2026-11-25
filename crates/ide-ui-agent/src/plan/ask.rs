//! The popover that takes a new comment on a plan or a specification, or a changed one.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSColor, NSLineBreakMode, NSPopover, NSPopoverBehavior, NSTextField, NSView, NSViewController};
use objc2_foundation::{NSRect, NSRectEdge};
use splat_i18n::{t, tf};

use super::super::{style, ui};
use super::entry_tag;
use crate::AgentKey;
use ide_ui_common::util::{ns, set_action};

const WIDTH: f64 = 300.0;

/// What the user wrote into the popover.
pub(super) struct Asked {
    /// The comment changed, or `None` for a new one.
    pub(crate) note: Option<u64>,
    pub(crate) location: usize,
    pub(crate) quote: String,
    pub(crate) comment: String,
}

/// What the popover asks about.
pub(super) struct Asking<'a> {
    /// What its buttons and its field send their actions to.
    pub(super) target: &'a AnyObject,
    /// The plan's entry, or the specification's number, which the controls' tags carry.
    pub(super) entry: u64,
    /// The comment changed, or `None` for a new one.
    pub(super) note: Option<u64>,
    pub(super) location: usize,
    pub(super) quote: String,
    /// What the field starts with.
    pub(super) current: &'a str,
    /// The heading of a comment that quotes nothing.
    pub(super) whole: &'a str,
}

/// A popover on view and what it asks about.
pub(super) struct Popover {
    note: Option<u64>,
    location: usize,
    quote: String,
    popover: Retained<NSPopover>,
    field: Retained<NSTextField>,
}

impl Popover {
    /// Closes it and hands over what it holds.
    pub(super) fn finish(self) -> Asked {
        self.popover.close();
        Asked {
            note: self.note,
            location: self.location,
            quote: self.quote,
            comment: self.field.stringValue().to_string(),
        }
    }

    /// Closes it, dropping what was typed.
    pub(super) fn dismiss(self) {
        self.popover.close();
    }
}

/// The popover on a plan of the AI tab `agent`.
struct Ask {
    agent: AgentKey,
    entry: u64,
    popover: Popover,
}

/// The question about a plan on view.
#[derive(Default)]
pub(super) struct PlanAsk {
    ask: RefCell<Option<Ask>>,
}

/// Shows the popover on plan `asking.entry` of the AI tab `agent` at `rect` of `anchor` as the question `ask` holds,
/// in place of the one open.
pub(super) fn open(
    ask: &PlanAsk,
    mtm: MainThreadMarker,
    agent: AgentKey,
    asking: Asking<'_>,
    anchor: &NSView,
    rect: NSRect,
) {
    close_in(ask);
    let entry = asking.entry;
    let popover = show(mtm, asking, anchor, rect);
    *ask.ask.borrow_mut() = Some(Ask { agent, entry, popover });
}

/// Shows a popover at `rect` of `anchor` for what `asking` names, which the caller keeps.
pub(super) fn show(mtm: MainThreadMarker, asking: Asking<'_>, anchor: &NSView, rect: NSRect) -> Popover {
    let Asking {
        target,
        entry,
        note,
        location,
        quote,
        current,
        whole,
    } = asking;
    let heading = if quote.trim().is_empty() {
        whole.to_string()
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
    set_action(&field, target, sel!(ideAgentPlanCommit:));
    field.setTag(tag);
    let cancel = ui::button(mtm, t("Abbrechen"), target, sel!(ideAgentPlanCancel:), tag);
    cancel.setKeyEquivalent(&ns("\u{1b}"));
    let title = if note.is_some() { t("Sichern") } else { t("Hinzufügen") };
    let add = ui::button(mtm, title, target, sel!(ideAgentPlanCommit:), tag);
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
    Popover {
        note,
        location,
        quote,
        popover,
        field,
    }
}

/// Closes the popover of plan `entry` in the AI tab `agent` that `ask` holds and hands over what it holds.
pub(super) fn take(ask: &PlanAsk, agent: AgentKey, entry: u64) -> Option<Asked> {
    let taken = {
        let mut a = ask.ask.borrow_mut();
        if a.as_ref().is_some_and(|ask| ask.agent == agent && ask.entry == entry) {
            a.take()
        } else {
            None
        }
    }?;
    Some(taken.popover.finish())
}

/// Closes the popover on view, dropping what was typed.
pub(crate) fn close() {
    close_in(&crate::host::state().plan.ask);
}

/// Closes the popover `ask` holds, dropping what was typed.
fn close_in(ask: &PlanAsk) {
    let open = ask.ask.borrow_mut().take();
    if let Some(open) = open {
        open.popover.dismiss();
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
