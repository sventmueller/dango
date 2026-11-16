//! The self-test's hooks of `ask.rs`.

use super::*;

/// Types `text` into the open popover, for the self-test; whether one is open.
pub fn type_comment(text: &str) -> bool {
    with_ask(|a| {
        a.borrow()
            .as_ref()
            .map(|ask| ask.popover.field.setStringValue(&ns(text)))
            .is_some()
    })
}

/// The open popover's content and its field's text, for the self-test.
pub fn view() -> Option<(Retained<NSView>, String)> {
    with_ask(|a| {
        let a = a.borrow();
        let ask = a.as_ref()?;
        let view = ask.popover.popover.contentViewController()?.view();
        Some((view, ask.popover.field.stringValue().to_string()))
    })
}
