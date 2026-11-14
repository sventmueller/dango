//! The self-test's hooks of `rows.rs`.

use super::*;

impl CiPanel {
    /// The titles of `key`'s context menu and whether each can be chosen, separators left out.
    pub fn menu_titles(&self, key: Key) -> Vec<(String, bool)> {
        self.menu_entries(key)
            .into_iter()
            .filter(|(_, action, _)| action.is_some())
            .map(|(title, _, enabled)| (title.to_string(), enabled))
            .collect()
    }
}
