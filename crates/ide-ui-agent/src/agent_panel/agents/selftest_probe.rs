//! The self-test's hooks of `agents.rs`.

use super::*;

impl Agents {
    /// The titles of the "new session" menu, for the self-test.
    pub fn menu_titles(&self) -> Vec<String> {
        session_menu(&self.target)
            .itemArray()
            .iter()
            .map(|i| {
                if i.isSeparatorItem() {
                    "—".to_string()
                } else {
                    i.title().to_string()
                }
            })
            .collect()
    }
}
