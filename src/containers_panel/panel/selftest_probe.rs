//! The self-test's hooks of `panel.rs`.

use super::*;

impl ContainersPanel {
    pub(crate) fn path(&self) -> Option<String> {
        self.ivars().path.borrow().clone()
    }

    /// Gives the panel `path` in place of the login shell's PATH, for the self-test.
    pub fn set_path(&self, path: &std::path::Path) {
        self.ivars().started.set(true);
        self.stop_watching();
        *self.ivars().path.borrow_mut() = Some(format!("{}:/usr/bin:/bin", path.display()));
        *self.ivars().reach.borrow_mut() = Reach::Unknown;
        self.ivars().failures.set(0);
        self.refresh();
    }

    /// Whether Docker can be used, as the panel shows it; for the self-test.
    pub fn reach(&self) -> Reach {
        self.ivars().reach.borrow().clone()
    }

    /// Whether a retry waits for the panel to show, for the self-test.
    pub fn waiting(&self) -> bool {
        self.ivars().waiting.get()
    }

    /// Whether the panel read Docker at all, for the self-test.
    pub fn started(&self) -> bool {
        self.ivars().started.get()
    }

    /// Whether the panel watches Docker's events, for the self-test.
    pub fn watching(&self) -> bool {
        self.ivars().events.get().is_some()
    }

    /// The panel's rows as their level, label, detail and look, and the note, for the self-test.
    pub fn shown(&self) -> (Vec<(usize, String, String, Look)>, String) {
        let rows = self
            .ivars()
            .rows
            .borrow()
            .iter()
            .map(|r| (r.level, r.label.clone(), r.detail.clone(), r.look))
            .collect();
        let note = self
            .ivars()
            .note
            .borrow()
            .as_ref()
            .map(|n| n.stringValue().to_string())
            .unwrap_or_default();
        (rows, note)
    }

    /// Selects the row labelled `label`, for the self-test.
    pub fn select(&self, label: &str) -> bool {
        let row = self.ivars().rows.borrow().iter().find(|r| r.label == label).cloned();
        match (row, self.ivars().table.borrow().as_ref()) {
            (Some(row), Some(table)) => {
                self.select_row(table, Some(&row));
                table.selectedRow() >= 0
            }
            _ => false,
        }
    }
}
