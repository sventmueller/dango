//! The self-test's hooks of `session.rs`.

use super::*;

/// What the record holds now, for the self-test.
pub fn recorded() -> Vec<WindowRecord> {
    State::load_user().get(&OPEN_WINDOWS)
}
