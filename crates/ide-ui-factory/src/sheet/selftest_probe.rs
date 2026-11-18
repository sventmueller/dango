//! The self-test's hooks of `sheet.rs`.

use super::*;

use crate::selftest::{SheetControl, SheetSeen, SheetTyped};

impl OrderSheet {
    /// The sheet "Neuer Auftrag" on `window`, while it is open: the target of its buttons.
    pub(crate) fn on(window: ide_ui_common::window::Window) -> Option<Retained<OrderSheet>> {
        let content = window.window()?.attachedSheet()?.contentView()?;
        find(&content)
    }

    /// What the sheet shows.
    pub(crate) fn seen(&self) -> SheetSeen {
        let ui = self.ui();
        let mut labels = Vec::new();
        if let Some(content) = ui.panel.contentView() {
            collect_labels(&content, &mut labels);
        }
        let messages = if ui.messages.isHidden() {
            Vec::new()
        } else {
            ui.messages
                .stringValue()
                .to_string()
                .lines()
                .map(str::to_string)
                .collect()
        };
        SheetSeen {
            title: ui.title.stringValue().to_string(),
            goal: ui.goal.string().to_string(),
            labels,
            messages,
            start: ui.start.isEnabled(),
            options: !ui.options.isHidden(),
            skip: ui.skip.isEnabled(),
            suggestions: (ui.suggestions.iter())
                .map(|tick| (tick.title().to_string(), tick.state() == NSControlStateValueOn))
                .collect(),
        }
    }

    /// Fills every field with `typed` and has the sheet check them, as typing does.
    pub(crate) fn type_in(&self, typed: &SheetTyped) {
        let ui = self.ui();
        ui.title.setStringValue(&ns(&typed.title));
        ui.goal.setString(&ns(&typed.goal));
        ui.criteria.setString(&ns(&typed.criteria));
        ui.scope.setStringValue(&ns(&typed.scope));
        ui.budget.setStringValue(&ns(&typed.budget));
        self.changed();
    }

    /// Clicks `control`, or picks its item; whether the sheet has it.
    pub(crate) fn press(&self, control: SheetControl) -> bool {
        let ui = self.ui();
        let button = match control {
            SheetControl::Risk(index) => {
                let Ok(at) = isize::try_from(index) else { return false };
                if at >= ui.risk.numberOfItems() {
                    return false;
                }
                ui.risk.selectItemAtIndex(at);
                // SAFETY: The popup's action is the sheet's `ideFactorySheetChanged:`, which takes any sender.
                unsafe { ui.risk.sendAction_to(ui.risk.action(), ui.risk.target().as_deref()) };
                return true;
            }
            SheetControl::Options => Some(ui.disclosure.clone()),
            SheetControl::Skip => Some(ui.skip.clone()),
            SheetControl::Suggestion(index) => ui.suggestions.get(index).cloned(),
            SheetControl::Cancel => {
                (ui.panel.contentView()).and_then(|view| button_with(&view, sel!(ideFactorySheetCancel:)))
            }
            SheetControl::Start => Some(ui.start.clone()),
        };
        let Some(button) = button else { return false };
        // SAFETY: A nil sender is what AppKit passes for a click made in code.
        unsafe { button.performClick(None) };
        true
    }
}

/// The sheet behind the buttons under `view`.
fn find(view: &NSView) -> Option<Retained<OrderSheet>> {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && let Some(sheet) = button.target().and_then(|target| target.downcast::<OrderSheet>().ok())
    {
        return Some(sheet);
    }
    view.subviews().iter().find_map(|child| find(&child))
}

fn button_with(view: &NSView, action: objc2::runtime::Sel) -> Option<Retained<NSButton>> {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && button.action() == Some(action)
    {
        return Some(button.retain());
    }
    view.subviews().iter().find_map(|child| button_with(&child, action))
}

fn collect_labels(view: &NSView, into: &mut Vec<String>) {
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        if !field.isEditable() && !field.isHidden() {
            into.push(field.stringValue().to_string());
        }
        return;
    }
    for child in view.subviews().iter() {
        collect_labels(&child, into);
    }
}
