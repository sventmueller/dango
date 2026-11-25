//! The self-test's hooks of `ask.rs`.

use super::*;

/// Runs `f` with the question on view.
fn with_ask<R>(f: impl FnOnce(&RefCell<Option<Ask>>) -> R) -> R {
    f(&crate::host::state().plan.ask.ask)
}

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

impl Popover {
    /// Types `text` into its field, for the self-test.
    pub(in crate::plan) fn type_text(&self, text: &str) {
        self.field.setStringValue(&ns(text));
    }

    /// What its field holds, for the self-test.
    pub(in crate::plan) fn field_text(&self) -> String {
        self.field.stringValue().to_string()
    }

    /// Its content, for the self-test.
    pub(in crate::plan) fn content(&self) -> Option<Retained<NSView>> {
        Some(self.popover.contentViewController()?.view())
    }
}
