//! The self-test's hooks of `view.rs`.

use super::*;

impl FileTreeSource {
    /// The row of `path` in `view`, -1 if it shows none.
    pub(crate) fn row_of(&self, view: &NSOutlineView, path: &Path) -> isize {
        let Some(item) = self.item_of(path) else { return -1 };
        // SAFETY: The item is one the data source handed to this view, kept alive in `items`.
        unsafe { view.rowForItem(Some(&item)) }
    }
}
