//! The self-test's hooks of `publish.rs`.

use super::*;

impl Leitstand {
    /// Carries out publishing's `effect` for the folder `root`, as a step of its factory would ask for it.
    pub(crate) fn publish_now(&self, root: &Path, effect: Effect) {
        self.publish_effect(root, effect);
    }
}
