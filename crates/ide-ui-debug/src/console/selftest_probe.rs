//! What the self-test reads from and does with the parts of `console.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl ConsoleTab {
    /// The text the console shows, for the self-test.
    pub fn shown_text(&self) -> String {
        self.text.string().to_string()
    }

    /// The field for expressions, for the self-test.
    pub fn input_field(&self) -> Retained<NSTextField> {
        self.input.clone()
    }

    /// Whether the field for expressions shows, for the self-test.
    pub fn takes_input(&self) -> bool {
        !self.input.isHidden()
    }
}
