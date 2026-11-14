//! The self-test's hooks of `whitespace.rs`.

use super::*;

/// Whether the overlay of `view` is showing, for the self-test.
pub fn visible(view: &EditorTextView) -> bool {
    view.ivars()
        .whitespace
        .borrow()
        .as_ref()
        .and_then(Weak::load)
        .is_some_and(|overlay| !overlay.isHidden())
}
