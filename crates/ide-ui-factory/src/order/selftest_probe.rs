//! The self-test's hooks of `order.rs`.

use super::*;

use crate::selftest::OrderSeen;

impl OrderTab {
    /// What the tab shows: the texts of its condition and body, its buttons and its commands.
    pub(crate) fn seen(&self) -> OrderSeen {
        let ui = self.ui();
        let mut seen = OrderSeen {
            texts: Vec::new(),
            buttons: Vec::new(),
            bar: ui
                .bar
                .arrangedSubviews()
                .iter()
                .filter(|view| view.downcast_ref::<NSPopUpButton>().is_none())
                .filter_map(|view| view.downcast_ref::<NSButton>().map(|b| b.title().to_string()))
                .collect(),
        };
        for view in [&*ui.condition as &NSView, &ui.body] {
            collect(view, &mut seen);
        }
        for item in &ui.items {
            seen.buttons.push((item.title().to_string(), item.isEnabled()));
        }
        seen
    }

    /// Runs `command` as its button does.
    pub(crate) fn press(&self, command: Command) {
        self.run(command);
    }
}

fn collect(view: &NSView, seen: &mut OrderSeen) {
    if let Some(button) = view.downcast_ref::<NSButton>() {
        seen.buttons.push((button.title().to_string(), button.isEnabled()));
        return;
    }
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        seen.texts.push(field.stringValue().to_string());
        return;
    }
    for child in view.subviews().iter() {
        collect(&child, seen);
    }
}
