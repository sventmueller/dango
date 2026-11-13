//! Comments on a plan with the keyboard alone: ⌥⌘K in the AI tab comments on the whole plan, and in the plan's card
//! and in its document a passage selected with the keyboard takes its comment with ⌥⌘K.

use std::path::PathBuf;
use std::time::Duration;

use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSEventModifierFlags, NSView, NSWindow};

use super::super::agent_plan::start_frames;
use super::{keys, window_of};
use crate::agent::AgentTab;
use crate::agent::plan;
use crate::selftest::check;
use crate::selftest::helpers::after;

/// How long the timeline takes to build a card it was just fed, which it tells nothing about.
const SETTLE: Duration = Duration::from_millis(600);
const WHOLE: &str = "Whole plan by keyboard";
const PASSAGE: &str = "Passage by keyboard";
const CARD: &str = "Card passage by keyboard";

/// Opens an AI tab that waits for a plan's approval, then the comments, then `then`.
pub fn start(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = window_of(&root) else { return };
    let Some(tab) = crate::agent::open_replay(&controller) else {
        check("an AI tab with a plan opens", false, "");
        return then(root);
    };
    tab.feed(&start_frames());
    tab.timeline().scroll_to_end();
    let key = tab.key;
    after(SETTLE, move || whole(root, key, then));
}

fn tab_of(key: u64) -> Option<std::rc::Rc<AgentTab>> {
    crate::agent::all().into_iter().find(|tab| tab.key == key)
}

/// The plan of the tab's card, and its comments: what each quotes and says.
fn notes(tab: &AgentTab) -> (Option<u64>, Vec<(String, String)>) {
    let inputs = tab.timeline().inputs().borrow();
    let id = inputs.plan_text.keys().copied().max();
    let notes = id
        .and_then(|id| inputs.plan_notes.get(&id))
        .map(|notes| notes.iter().map(|n| (n.quote.clone(), n.comment.clone())).collect())
        .unwrap_or_default();
    (id, notes)
}

/// Presses the shortcut of "Plan kommentieren …", types `comment` into the popover and Return; where the shortcut went.
fn comment(window: &NSWindow, comment: &str) -> String {
    let Some((key, modifiers)) = keys::shortcut_of(window, objc2::sel!(idePlanComment:)) else {
        return "no menu item".into();
    };
    let went = keys::press(window, &key, 0, modifiers).to_string();
    let Some(popover) = plan::comment_view().and_then(|(view, _)| view.window()) else {
        return format!("{went}, no popover");
    };
    keys::type_keys(&popover, comment);
    let _ = keys::press(&popover, "\r", 36, NSEventModifierFlags::empty());
    went
}

/// ⌥⌘K with the focus in the AI tab comments on the whole plan.
fn whole(root: PathBuf, key: u64, then: fn(PathBuf)) {
    let (Some(window), Some(tab)) = (window_of(&root).and_then(|c| c.window()), tab_of(key)) else {
        return then(root);
    };
    // The way into a message field by keyboard, ⌘Esc, is the prompt flow's to check; here the focus starts there.
    let inside = keys::focused(&window).is_some_and(|focus| focus.isDescendantOf(&tab.root()));
    if !inside {
        tab.select();
        tab.focus_composer();
    }
    let went = comment(&window, WHOLE);
    let (id, notes) = notes(&tab);
    check(
        "⌥⌘K in the AI tab comments on its plan",
        notes.iter().any(|(quote, said)| quote.is_empty() && said == WHOLE),
        format!("to {went}; {notes:?}"),
    );
    let Some(id) = id else { return then(root) };
    after(SETTLE, move || in_card(root, key, id, then));
}

/// The view with the focus, if it is `view`.
fn has_focus(window: &NSWindow, view: &NSView) -> bool {
    keys::focused(window).is_some_and(|focus| std::ptr::eq(&*focus, view))
}

/// With the plan's row selected in the timeline, Tab reaches the plan's text in its card and ⇧Tab goes back; there
/// ⇧⌥→ selects the first words and ⌥⌘K comments on them.
fn in_card(root: PathBuf, key: u64, id: u64, then: fn(PathBuf)) {
    let (Some(window), Some(tab)) = (window_of(&root).and_then(|c| c.window()), tab_of(key)) else {
        return then(root);
    };
    // The timeline itself is in the window's Tab order, which the walks of the AI tab's states check.
    let table = tab.timeline().table().retain();
    window.makeFirstResponder(Some(&table));
    for _ in 0..table.numberOfRows() {
        if tab.timeline().plan_text_of_row(table.selectedRow()).is_some() {
            break;
        }
        let _ = keys::press(&window, "\u{f701}", 125, NSEventModifierFlags::empty());
    }
    let text = tab.timeline().plan_text_of_row(table.selectedRow());
    let card = text
        .as_ref()
        .map(|t| Retained::into_super(Retained::into_super(t.clone())));
    let _ = keys::press(&window, "\t", 48, NSEventModifierFlags::empty());
    let reached = card.as_ref().is_some_and(|card| has_focus(&window, card));
    let _ = keys::press(&window, "\t", 48, NSEventModifierFlags::Shift);
    let back = has_focus(&window, &table);
    check(
        "Tab from the timeline reaches the plan's text in its card, and ⇧Tab goes back",
        reached && back,
        format!("row {} selected, reached {reached}, back {back}", table.selectedRow()),
    );
    let _ = keys::press(&window, "\t", 48, NSEventModifierFlags::empty());
    let words = NSEventModifierFlags::Shift | NSEventModifierFlags::Option;
    for _ in 0..2 {
        let _ = keys::press(&window, "\u{f703}", 124, words);
    }
    let selected = text.as_ref().map_or(0, |text| text.selectedRange().length);
    let went = comment(&window, CARD);
    let (_, notes) = notes(&tab);
    check(
        "⌥⌘K comments on a passage selected with the keyboard in the plan's card",
        notes
            .iter()
            .any(|(quote, said)| !quote.trim().is_empty() && said == CARD),
        format!("{selected} characters selected, to {went}; {notes:?}"),
    );
    tab.open_plan(id);
    after(SETTLE, move || passage(root, key, id, then));
}

/// In the plan's document, ⇧⌥→ twice selects the first words and ⌥⌘K comments on them.
fn passage(root: PathBuf, key: u64, id: u64, then: fn(PathBuf)) {
    let (Some(window), Some(tab)) = (window_of(&root).and_then(|c| c.window()), tab_of(key)) else {
        return then(root);
    };
    let text = plan::text_of(key, id);
    let in_document = text.as_ref().is_some_and(|text| {
        let view: &NSView = text;
        keys::focused(&window).is_some_and(|focus| std::ptr::eq(&*focus, view))
    });
    let words = NSEventModifierFlags::Shift | NSEventModifierFlags::Option;
    for _ in 0..2 {
        let _ = keys::press(&window, "\u{f703}", 124, words);
    }
    let selected = text.as_ref().map_or(0, |text| text.selectedRange().length);
    let went = comment(&window, PASSAGE);
    let (_, notes) = notes(&tab);
    check(
        "⌥⌘K comments on a passage selected with the keyboard in the plan's document",
        in_document
            && notes
                .iter()
                .any(|(quote, said)| !quote.trim().is_empty() && said == PASSAGE),
        format!("focus in the document {in_document}, {selected} characters selected, to {went}; {notes:?}"),
    );
    then(root);
}
