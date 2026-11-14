//! The self-test's hooks of `panel.rs`.

use super::*;

impl TestsPanel {
    /// The panel's rows as their indentation, label and state, for the self-test.
    pub fn shown(&self) -> Vec<(usize, String, State)> {
        self.ivars()
            .rows
            .borrow()
            .iter()
            .map(|r| (r.level, r.label.clone(), r.state))
            .collect()
    }
}
