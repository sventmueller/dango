//! The self-test's hooks of `registry.rs`.

use super::*;

/// The open windows, for the self-test.
pub(crate) fn open_windows() -> Vec<Retained<WindowController>> {
    crate::context::app().windows().all()
}
