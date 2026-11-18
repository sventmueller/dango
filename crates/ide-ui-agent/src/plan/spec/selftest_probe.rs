//! The self-test's hooks of `spec.rs`.

use objc2::runtime::Sel;
use objc2_app_kit::NSStackView;

use super::*;

/// A button in the head of a specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecButton {
    Comment,
    Discard,
    Send,
    Approve,
}

/// What a comment in the specification's column offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecNoteAction {
    Show,
    Edit,
    Remove,
}

/// What an open specification shows, for the self-test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSeen {
    /// The ids of its comments, in the order of the column.
    pub notes: Vec<u64>,
    /// The boxes in its comments' column.
    pub boxes: usize,
    /// The passages marked in its text.
    pub marked: usize,
    /// Whether Comment, Discard, Send the comments and Approve take a click, in that order.
    pub enabled: [bool; 4],
    /// What its head says about it.
    pub state: String,
    /// What the comment popover's field holds, while one is open.
    pub asking: Option<String>,
    /// What is selected in its text.
    pub selected: String,
}

fn doc_of(view: &NSView) -> Option<Rc<SpecDoc>> {
    view.downcast_ref::<SpecRoot>()?.ivars().doc.get().cloned()
}

/// What the specification `view`, made by `spec_view`, shows.
pub fn spec_seen(view: &NSView) -> Option<SpecSeen> {
    let doc = doc_of(view)?;
    let text = &doc.frame.text;
    // SAFETY: The text view made by `scrollableTextView` owns its text system; asking for the storage keeps TextKit 2.
    let storage = unsafe { text.textStorage() }?;
    let string = text.string();
    let range = text.selectedRange();
    let selected = if range.location + range.length <= string.length() {
        string.substringWithRange(range).to_string()
    } else {
        String::new()
    };
    let boxes = doc
        .frame
        .notes
        .arrangedSubviews()
        .firstObject()
        .and_then(|list| list.downcast::<NSStackView>().ok())
        .map_or(0, |list| list.arrangedSubviews().count());
    Some(SpecSeen {
        notes: doc.notes.borrow().iter().map(|n| n.id).collect(),
        boxes,
        marked: super::super::count_marks(&storage),
        enabled: doc.buttons.each_ref().map(|b| b.isEnabled()),
        state: doc.frame.state.stringValue().to_string(),
        asking: doc.asking.borrow().as_ref().map(Popover::field_text),
        selected,
    })
}

/// Marks the first `needle` in the specification `view` as the user would, an empty one marking nothing; whether it
/// is there.
pub fn spec_select(view: &NSView, needle: &str) -> bool {
    let Some(doc) = doc_of(view) else { return false };
    let string = doc.frame.text.string().to_string();
    let Some(at) = string.find(needle) else { return false };
    let range = NSRange::new(string[..at].encode_utf16().count(), needle.encode_utf16().count());
    doc.frame.text.setSelectedRange(range);
    true
}

/// Clicks `button` in the head of the specification `view`; whether it took the click.
pub fn spec_press(view: &NSView, button: SpecButton) -> bool {
    let Some(doc) = doc_of(view) else { return false };
    let button = doc.buttons[button as usize].clone();
    let enabled = button.isEnabled();
    press(Some(button));
    enabled
}

/// Types `text` into the comment popover of the specification `view`; whether one is open.
pub fn spec_type(view: &NSView, text: &str) -> bool {
    doc_of(view).is_some_and(|doc| doc.asking.borrow().as_ref().map(|p| p.type_text(text)).is_some())
}

/// Clicks Add or Save of the comment popover of the specification `view` where `commit`, else Cancel; whether one was
/// open.
pub fn spec_answer_popover(view: &NSView, commit: bool) -> bool {
    let Some(doc) = doc_of(view) else { return false };
    let action = if commit {
        sel!(ideAgentPlanCommit:)
    } else {
        sel!(ideAgentPlanCancel:)
    };
    let content = doc.asking.borrow().as_ref().and_then(Popover::content);
    press(content.and_then(|content| button_in(&content, action, None)))
}

/// Clicks what `action` names on the comment `note` in the column of the specification `view`; whether it is there.
pub fn spec_note(view: &NSView, note: u64, action: SpecNoteAction) -> bool {
    let Some(doc) = doc_of(view) else { return false };
    let action = match action {
        SpecNoteAction::Show => sel!(ideAgentPlanShowNote:),
        SpecNoteAction::Edit => sel!(ideAgentPlanEditNote:),
        SpecNoteAction::Remove => sel!(ideAgentPlanRemoveNote:),
    };
    let button = button_in(&doc.frame.notes, action, isize::try_from(note).ok());
    press(button)
}

fn button_in(view: &NSView, action: Sel, tag: Option<isize>) -> Option<Retained<NSButton>> {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && button.action() == Some(action)
        && tag.is_none_or(|tag| button.tag() == tag)
    {
        return Some(button.retain());
    }
    view.subviews().iter().find_map(|child| button_in(&child, action, tag))
}

fn press(button: Option<Retained<NSButton>>) -> bool {
    button.is_some_and(|button| {
        // SAFETY: A click without a sender, on the main thread, sends the button's action as a real click does.
        unsafe { button.performClick(None) };
        true
    })
}
