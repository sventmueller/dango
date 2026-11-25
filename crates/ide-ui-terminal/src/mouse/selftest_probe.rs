//! The self-test's hooks of `mouse.rs`.

use super::*;

impl TerminalView {
    /// Whether the pointing hand shows, for the self-test.
    pub(crate) fn pointing_hand(&self) -> bool {
        self.ivars().mouse.hand.get()
    }
}
