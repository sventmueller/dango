//! What the self-test reads from and does with the parts of `ui.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Ui {
    /// Whether the empty state offers the browser login, and whether it can be pressed.
    pub fn browser_button(&self) -> Option<bool> {
        self.browser.get()
    }

    /// The note below the list, when it shows.
    pub fn note(&self) -> Option<String> {
        (!self.no_runs.isHidden()).then(|| self.no_runs.stringValue().to_string())
    }
}
