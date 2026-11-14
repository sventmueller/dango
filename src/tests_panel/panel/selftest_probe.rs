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

    /// Whether the panel shows its tree rather than the hint that no test ran yet, for the self-test.
    pub fn tree_shown(&self) -> bool {
        self.ivars()
            .scroll
            .borrow()
            .as_ref()
            .is_some_and(|scroll| !scroll.isHidden())
    }
}
