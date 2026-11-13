//! What the self-test reads from and does with the parts of `shown.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The text of the box under the line, if one shows, for the self-test.
pub fn boxed_text(view: &EditorTextView) -> Option<String> {
    let state = view.ivars().predict.borrow();
    state.parts.field.as_ref().map(|field| field.stringValue().to_string())
}

/// The range of the text marked as changing, for the self-test.
pub fn marked(view: &EditorTextView) -> Option<Range<usize>> {
    view.ivars().predict.borrow().parts.marked.clone()
}

/// The status bar's switch: its title and whether it is enabled, for the self-test.
pub fn status_shown(view: &EditorTextView) -> Option<(String, bool)> {
    let state = view.ivars().predict.borrow();
    state
        .parts
        .button
        .as_ref()
        .map(|button| (button.title().to_string(), button.isEnabled()))
}
