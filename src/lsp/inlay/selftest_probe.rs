//! The self-test's hooks of `inlay.rs`.

use super::*;

/// The hints `view` shows, as their offsets and texts, for the self-test.
pub fn shown(view: &EditorTextView) -> Vec<(usize, String, bool)> {
    insets::of(view, Layer::Inlay)
        .into_iter()
        .map(|inset| (inset.at, inset.text, inset.action.is_some()))
        .collect()
}
