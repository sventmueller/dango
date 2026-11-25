//! The self-test's hooks of `figures.rs`.

use super::*;

/// Whether the answers asked for `key`, for the self-test.
pub fn asked_for(key: &Key) -> bool {
    let state = crate::host::state();
    let asked = state.figures.asked.borrow();
    asked.as_ref().is_some_and(|asked| asked.keys.contains(key))
}
