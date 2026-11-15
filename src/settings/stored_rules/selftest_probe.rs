//! What the self-test reads from and does with the parts of `stored_rules.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl StoredRules {
    /// Selects the row `row` and removes its rule, as the user does, for the self-test.
    pub(crate) fn remove_row(&self, row: usize) {
        if let Some(table) = self.ivars().table.get() {
            table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        self.remove_selected();
    }

    /// Scrolls the Agents tab to the section's last line, for the self-test's picture.
    pub(crate) fn reveal_end(&self) {
        if let Some(end) = self.ivars().end.get() {
            end.scrollRectToVisible(end.bounds());
        }
    }
}
