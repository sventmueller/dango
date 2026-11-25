//! What the self-test reads from and does with the folder of a window, its tree, search and Open Quickly, kept out of
//! the production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl WindowController {
    /// The project search of the files panel.
    pub(crate) fn search_panel(&self) -> Option<Retained<SearchPanel>> {
        self.ivars().folder.search.borrow().clone()
    }

    pub(crate) fn tree_rows(&self) -> isize {
        self.ivars()
            .folder
            .files
            .borrow()
            .as_ref()
            .map_or(0, FileTree::row_count)
    }

    /// Opens Open Quickly with `query` typed into it.
    pub(crate) fn quick_open_with(&self, query: &str) {
        self.show_quick_open("");
        if let Some(panel) = self.ivars().folder.quick_open.borrow().as_ref() {
            panel.type_query(query);
        }
    }

    pub(crate) fn close_quick_open(&self) {
        if let Some(panel) = self.ivars().folder.quick_open.borrow().as_ref() {
            panel.close();
        }
    }

    /// What Open Quickly says in place of rows, if anything.
    pub(crate) fn quick_open_status(&self) -> Option<String> {
        self.ivars()
            .folder
            .quick_open
            .borrow()
            .as_ref()
            .and_then(|p| p.status())
    }

    /// Opens row `row` of Open Quickly as Return does.
    pub(crate) fn quick_open_row(&self, row: usize) {
        let panel = self.ivars().folder.quick_open.borrow().clone();
        if let Some(panel) = panel {
            panel.open_row(row);
        }
    }

    pub(crate) fn quick_open_hits(&self) -> Vec<String> {
        self.ivars()
            .folder
            .quick_open
            .borrow()
            .as_ref()
            .map(|p| p.hits())
            .unwrap_or_default()
    }

    /// Selects the row of `path` in the folder tree, as a click on it does.
    pub(crate) fn select_in_tree(&self, path: &Path) {
        let files = self.ivars().folder.files.borrow();
        if let Some(files) = files.as_ref() {
            files.select(path);
        }
    }

    /// Clicks the row of `path` in the folder tree, twice for a double click.
    pub(crate) fn click_in_tree(&self, path: &Path, double: bool) {
        let files = self.ivars().folder.files.borrow();
        if let Some(files) = files.as_ref() {
            files.click(path, double);
        }
    }
}
