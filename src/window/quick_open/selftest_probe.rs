//! What the self-test reads from and does with the parts of `quick_open.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl QuickOpenPanel {
    pub(crate) fn hits(&self) -> Vec<String> {
        match &*self.ivars().hits.borrow() {
            Rows::Files(hits) => hits.iter().map(|hit| hit.path.clone()).collect(),
            Rows::Symbols(symbols) => symbols
                .iter()
                .map(|found| {
                    let path = found.path.strip_prefix(&self.ivars().root).unwrap_or(&found.path);
                    format!("{} {}", found.symbol.name, path.display())
                })
                .collect(),
        }
    }

    /// What the list says in place of rows, if anything.
    pub(crate) fn status(&self) -> Option<String> {
        let status = self.ivars().status.borrow();
        let status = status.as_ref()?;
        (!status.isHidden()).then(|| status.stringValue().to_string())
    }

    /// Opens row `row` as Return does.
    pub(crate) fn open_row(&self, row: usize) {
        if let Some(table) = self.ivars().table.borrow().as_ref() {
            table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        self.open_selected();
    }

    pub(crate) fn type_query(&self, query: &str) {
        if let Some(field) = self.ivars().field.borrow().as_ref() {
            field.setStringValue(&ns(query));
        }
        self.ask();
    }
}
