//! The self-test's hooks of `hover.rs`.

use super::*;

/// The text of the popover shown on `view`, for the self-test.
pub fn shown_text(view: &EditorTextView) -> Option<String> {
    let state = view.ivars().lsp.borrow();
    state
        .hover
        .popover
        .as_ref()
        .filter(|p| p.isShown())
        .map(|_| state.hover.shown.clone())
}
