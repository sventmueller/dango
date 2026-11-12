//! Finding in a window's folder: Open Quickly, the project search, the call and type hierarchies, and the files panel's
//! tree they reveal into.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};

use super::{Panel, WindowController, files::FileTree, quick_open::QuickOpenPanel};

impl WindowController {
    pub(super) fn show_quick_open(&self, query: &str) {
        let Some(root) = self.ivars().workspace.borrow().as_ref().map(|p| p.root().to_path_buf()) else {
            return;
        };
        let Some(window) = self.window() else { return };
        let panel = self
            .ivars()
            .quick_open
            .borrow_mut()
            .get_or_insert_with(|| QuickOpenPanel::new(self.mtm(), self, root))
            .clone();
        panel.show(&window, query);
    }

    /// Shows the search panel with the keyboard focus in its field (⌘⇧F).
    pub fn show_search(&self) {
        let Some(search) = self.ivars().search.borrow().clone() else {
            return;
        };
        if !self.ivars().panel_open.get() {
            self.set_panel_open(true);
        }
        self.show_panel(Panel::Search);
        search.focus();
    }

    /// Shows `hierarchy` in the search panel in place of the text search (E85).
    pub fn show_hierarchy(&self, hierarchy: crate::lsp::Hierarchy) {
        let Some(search) = self.ivars().search.borrow().clone() else {
            return;
        };
        if !self.ivars().panel_open.get() {
            self.set_panel_open(true);
        }
        self.show_panel(Panel::Search);
        search.show_hierarchy(hierarchy);
    }

    /// Asks the folder's index for up to `limit` files matching `query`; `answer` runs on another thread.
    pub fn query_files(&self, query: &str, limit: usize, answer: impl FnOnce(Vec<ide_project::Hit>) + Send + 'static) {
        let workspace = self.ivars().workspace.borrow();
        if let Some(quick_open) = workspace.as_ref().and_then(|p| p.quick_open()) {
            quick_open.query(query, limit, answer);
        }
    }

    /// The outline view of the folder tree, for its context menu.
    pub(super) fn file_tree_view(&self) -> Option<Retained<objc2_app_kit::NSOutlineView>> {
        self.ivars().files.borrow().as_ref().map(FileTree::outline)
    }
}
