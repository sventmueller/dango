//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl FileTree {
    pub(crate) fn row_count(&self) -> isize {
        self.view.numberOfRows()
    }

    /// Selects the row of `path`, as a click on it does.
    pub(crate) fn select(&self, path: &std::path::Path) {
        if let Ok(row) = usize::try_from(self.source.row_of(&self.view, path)) {
            self.view
                .selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
    }

    /// Clicks the row of `path`, twice for a double click, as the mouse does.
    pub(crate) fn click(&self, path: &std::path::Path, double: bool) {
        let row = self.source.row_of(&self.view, path);
        self.source.row_clicked(&self.view, row);
        if double {
            self.source.row_double_clicked(&self.view, row);
        }
    }
}

impl FileTree {
    /// The titles of the menu an Option-Shift click on a file opens, for the panes `panes`.
    pub(crate) fn pane_menu_titles(&self, panes: &[(usize, Option<String>)]) -> Vec<String> {
        let menu = self.source.pane_menu(panes);
        menu.itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| item.title().to_string())
            .collect()
    }
}
