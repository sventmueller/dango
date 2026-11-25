//! The self-test's hooks of `mod.rs`.

use super::*;

/// The panel of `controller`'s window, for the self-test.
pub(crate) fn panel_of(controller: &WindowController) -> Option<Retained<TestsPanel>> {
    of(controller)
}
