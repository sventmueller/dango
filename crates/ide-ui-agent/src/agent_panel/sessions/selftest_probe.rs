//! What the self-test reads from and does with the parts of `sessions.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Source {
    /// The context menu of row `row`, as a right click on it fills it, for the self-test.
    pub(crate) fn context_menu(&self, row: isize) -> Option<Retained<NSMenu>> {
        let panel = super::super::panel_by_key(self.ivars().panel.get())?;
        let menu = NSMenu::new(self.mtm());
        self.fill_menu_for(&menu, &panel, row);
        Some(menu)
    }

    /// Opens the session of row `row`, as a click on it does, for the self-test.
    pub(crate) fn open_row(&self, row: usize) {
        let id = self.ivars().model.borrow().order.get(row).cloned();
        if let Some(id) = id {
            self.open_id(&id);
        }
    }

    /// The quiet texts at the end of the session rows, for the self-test.
    pub(crate) fn session_details(&self) -> Vec<String> {
        let model = self.ivars().model.borrow();
        model
            .nodes
            .values()
            .filter_map(|node| match node {
                Node::Thread { entry, .. } => Some(crate::short_age(entry.last_activity)),
                _ => None,
            })
            .collect()
    }

    /// The rows top to bottom, for the self-test: "AI" or "CLI", whether open, and the title.
    pub(crate) fn rows(&self) -> Vec<(&'static str, bool, String)> {
        let model = self.ivars().model.borrow();
        model
            .order
            .iter()
            .filter_map(|id| model.nodes.get(id))
            .map(|node| match node {
                Node::Thread { entry, .. } => ("AI", node.open(), entry.title.clone()),
                Node::Past { resume, .. } => ("CLI", false, resume.title.clone().unwrap_or_default()),
            })
            .collect()
    }
}
