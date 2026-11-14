//! The self-test's hooks of `page.rs`.

use super::*;

impl Views {
    /// Whether the offer shows.
    pub fn offers_web_pictures(&self) -> bool {
        !self.bar.isHidden()
    }
}
