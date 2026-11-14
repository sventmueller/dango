//! The self-test's hooks of `controls.rs`.

use super::*;

impl Mic {
    pub fn spinning(&self) -> bool {
        !self.spinner.isHidden()
    }
}
