//! The self-test's hooks of `sheet.rs`.

use super::*;

impl Sheet {
    /// What the sheet says now.
    pub fn text(&self) -> String {
        self.label.stringValue().to_string()
    }
}
