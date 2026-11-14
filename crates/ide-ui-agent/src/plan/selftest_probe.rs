//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// Marks the first `needle` in plan `id`'s card or document (`place`) as the user would; whether it is there.
pub fn select(tab: &AgentTab, id: u64, place: u8, needle: &str) -> bool {
    let text = match place {
        PLAN_IN_DOC => doc::text_of(tab.key, id),
        _ => tab.timeline().inputs().borrow().plan_text.get(&id).cloned(),
    };
    let Some(text) = text else { return false };
    let string = text.string().to_string();
    let Some(at) = string.find(needle) else { return false };
    let range = NSRange::new(string[..at].encode_utf16().count(), needle.encode_utf16().count());
    text.setSelectedRange(range);
    true
}

pub use super::ask::{type_comment, view as comment_view};

pub use super::doc::{inspect, text_of};

/// How many passages `text` marks, for the self-test.
pub(super) fn count_marks(text: &NSAttributedString) -> usize {
    let key = ide_ui_common::constant!(objc2_app_kit::NSToolTipAttributeName);
    let mut marks = 0;
    let mut at = 0;
    while at < text.length() {
        let mut effective = NSRange::new(0, 0);
        // SAFETY: `at` lies inside the text and the effective range is a valid place to write to.
        let value = unsafe { text.attribute_atIndex_effectiveRange(key, at, &mut effective) };
        marks += usize::from(value.is_some());
        at = (effective.location + effective.length).max(at + 1);
    }
    marks
}

/// How many passages plan `id`'s card marks, for the self-test.
pub fn card_marks(tab: &AgentTab, id: u64) -> Option<usize> {
    let text = tab.timeline().inputs().borrow().plan_text.get(&id).cloned()?;
    // SAFETY: A card's text view owns its storage, which is TextKit 1 from the start.
    let storage = unsafe { text.textStorage() }?;
    Some(count_marks(&storage))
}

/// Whether plan `id`'s card is in the window, which a card only measured is not yet.
pub fn card_shown(tab: &AgentTab, id: u64) -> bool {
    let text = tab.timeline().inputs().borrow().plan_text.get(&id).cloned();
    text.is_some_and(|text| text.window().is_some())
}
