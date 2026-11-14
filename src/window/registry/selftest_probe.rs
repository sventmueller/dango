//! The self-test's hooks of `registry.rs`.

use super::*;

/// The open windows, for the self-test.
pub fn open_windows() -> Vec<Retained<WindowController>> {
    windows()
}
