//! The self-test's hooks of `rows.rs`.

use super::*;

/// The replace button of a row the self-test asks for.
pub(crate) fn button_of(view: &NSView) -> Option<Retained<NSButton>> {
    view.downcast_ref::<ResultCell>()?.ivars().button.borrow().clone()
}

/// What VoiceOver reads for a row made here: the one text of the row that has a label of its own.
pub(crate) fn spoken(view: &NSView) -> Option<String> {
    view.subviews().iter().find_map(|subview| {
        let field = subview.downcast_ref::<NSTextField>()?;
        field.accessibilityLabel().map(|label| label.to_string())
    })
}
