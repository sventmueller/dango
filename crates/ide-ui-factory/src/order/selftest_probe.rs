//! The self-test's hooks of `order.rs`.

use super::*;

use crate::selftest::OrderSeen;

impl OrderTab {
    /// What the tab shows: the texts of its condition and body, and its buttons.
    pub(crate) fn seen(&self) -> OrderSeen {
        let mut seen = OrderSeen {
            texts: Vec::new(),
            buttons: Vec::new(),
        };
        let ui = self.ui();
        for view in [&*ui.condition as &NSView, &ui.body] {
            collect(view, &mut seen);
        }
        for button in &ui.buttons {
            seen.buttons.push((button.title().to_string(), button.isEnabled()));
        }
        seen
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
