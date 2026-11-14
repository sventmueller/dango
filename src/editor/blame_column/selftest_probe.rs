//! The self-test's hooks of `blame_column.rs`.

use super::*;

/// The column of `view`, for the self-test.
pub fn of(view: &EditorTextView) -> Option<Retained<BlameColumn>> {
    column_of(view)
}

impl BlameColumn {
    /// The entries on screen as their first line and text, for the self-test.
    pub fn shown(&self) -> Vec<(usize, String)> {
        self.ivars()
            .editor
            .load()
            .map(|editor| self.entries(&editor).into_iter().map(|e| (e.line, e.text)).collect())
            .unwrap_or_default()
    }
}
