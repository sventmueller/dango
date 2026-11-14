//! The self-test's hooks of `ruler.rs`.

use super::*;

impl LineNumbers {
    /// Whether the mouse is over the column, for the self-test.
    pub(crate) fn hovered(&self) -> bool {
        self.ivars().hovered.get()
    }
}
