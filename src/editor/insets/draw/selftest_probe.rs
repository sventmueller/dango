//! The self-test's hooks of `draw.rs`.

use super::*;

/// How many insets the overlay draws when AppKit asks it for `rect`, for the self-test.
pub fn drawn_in(view: &EditorTextView, rect: NSRect) -> usize {
    shown_in(view, rect).map_or(0, |shown| shown.len())
}
