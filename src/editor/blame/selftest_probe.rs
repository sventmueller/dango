//! The self-test's hooks of `blame.rs`.

use super::*;

/// The text of the open popover's labels, for the self-test.
pub fn popover_texts(view: &EditorTextView) -> Option<Vec<String>> {
    pub(crate) fn collect(view: &NSView, out: &mut Vec<String>) {
        if let Some(field) = view.downcast_ref::<NSTextField>() {
            out.push(field.stringValue().to_string());
        }
        if let Some(button) = view.downcast_ref::<NSButton>() {
            out.push(button.title().to_string());
        }
        for child in view.subviews().iter() {
            collect(&child, out);
        }
    }
    let popover = view.ivars().blame.borrow().popover.clone()?;
    if !popover.isShown() {
        return None;
    }
    let mut out = Vec::new();
    collect(&popover.contentViewController()?.view(), &mut out);
    Some(out)
}
