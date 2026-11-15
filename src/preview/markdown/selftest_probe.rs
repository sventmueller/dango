//! The self-test's hooks of `markdown.rs`.

use super::*;

impl Preview {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
