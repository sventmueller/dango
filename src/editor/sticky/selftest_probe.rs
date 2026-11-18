//! The self-test's hooks of `sticky.rs`.

use super::*;

/// The header lines `view` shows, for the self-test.
pub(crate) fn shown(view: &EditorTextView) -> Vec<usize> {
    of(view).map(|s| s.ivars().shown.borrow().clone()).unwrap_or_default()
}

/// Clicks the header row of `view` at `y` from the top, for the self-test.
pub(crate) fn click(view: &EditorTextView, y: f64) {
    if let Some(sticky) = of(view) {
        sticky.click_row(y);
    }
}
