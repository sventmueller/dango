//! The self-test's hooks of `vim.rs`.

use super::*;

/// The mode label of `view`'s status bar and whether the system caret is hidden for the block, for the self-test.
pub fn shown(view: &EditorTextView) -> (String, bool) {
    let label = view
        .ivars()
        .vim
        .borrow()
        .label
        .as_ref()
        .map(|l| l.stringValue().to_string())
        .unwrap_or_default();
    let hidden = view.insertionPointColor().alphaComponent() == 0.0;
    (label, hidden)
}
