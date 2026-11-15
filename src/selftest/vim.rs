//! The `editor-vim` scenario (B26): Editor ▸ Vim-Modus puts the editor in Normal mode with a block caret and the mode in
//! the status bar; keys through the input context run motions and operators, `ciw…Esc` and `.` repeat, `u` undoes on
//! the editor's history and an insert pass of several lines is one undo step; a Japanese input method types in Insert
//! mode; `^` of a German keyboard goes to the line's first word; ⌃R redoes only in the editor with the focus, also with
//! a second editor beside it; and an agent's change during an insert pass is an undo step of its own.

use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSTextInputClient};
use objc2_foundation::{NSPoint, NSRange};

use super::helpers::{self, wait_until};
use super::{check, finish, give_up, snapshot};
use crate::editor::{self, EditorTextView};
use crate::util::ns;
use crate::window;

const SCENARIO: &str = "editor-vim";
const PROJECT: &str = "vim-project";
const FILE: &str = "notes.txt";
const OTHER: &str = "other.txt";
const TEXT: &str = "  alpha beta gamma\nsecond line\nthird line\n";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn editor_of(file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(file))
}

fn text(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

/// Types `keys` as the input context hands them over: printable characters as text, Esc as its command.
fn keys(view: &EditorTextView, keys: &str) {
    for part in keys.split("<Esc>").enumerate().flat_map(|(n, piece)| {
        let esc = (n > 0).then_some(None);
        esc.into_iter().chain(std::iter::once(Some(piece)))
    }) {
        match part {
            None => command(view, objc2::sel!(cancelOperation:)),
            Some(piece) => {
                for c in piece.chars() {
                    editor::type_text(view, &c.to_string());
                }
            }
        }
    }
}

fn command(view: &EditorTextView, selector: Sel) {
    // SAFETY: A selector of the text system, sent as the input context sends it.
    unsafe { view.doCommandBySelector(selector) };
}

/// A key with ⌃, as the window hands it to the first responder's `keyDown:`.
fn control(view: &EditorTextView, c: char) {
    let window_number = view.window().map_or(0, |w| w.windowNumber());
    let event = {
        NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::Control,
            0.0,
            window_number,
            None,
            &ns(&c.to_string()),
            &ns(&c.to_string()),
            false,
            0,
        )
    };
    if let Some(event) = event {
        view.keyDown(&event);
    }
}

fn picture(name: &str) {
    if let Some(window) = window::for_project(&root()).and_then(|c| c.window()) {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

pub(crate) fn run() {
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, TEXT), (OTHER, "other\n")], |controller| {
        controller.open_file(&root().join(FILE));
        wait_until(|| editor_of(FILE).is_some(), |_| normal_mode());
    });
}

fn normal_mode() {
    let Some(view) = editor_of(FILE) else {
        return give_up(SCENARIO, "the file opens in an editor");
    };
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(&view));
    }
    view.setSelectedRange(NSRange::new(0, 0));
    editor::toggle_vim();
    let (label, block) = editor::vim_shown(&view);
    check(
        "Editor ▸ Vim-Modus starts in Normal mode with a block caret and the mode in the status bar",
        label == "NORMAL" && block,
        format!("{label:?}, block {block}"),
    );
    picture("vim-normal");
    keys(&view, "wdw");
    let deleted = text(&view);
    keys(&view, "u");
    let undone = text(&view);
    check(
        "w and dw act on the text, and u takes the edit back from the editor's history",
        deleted == "  beta gamma\nsecond line\nthird line\n" && undone == TEXT,
        format!("{deleted:?} / {undone:?}"),
    );
    keys(&view, "ciwALPHA<Esc>w.");
    check(
        "ciw…Esc and . repeat the change on the next word",
        text(&view).starts_with("  ALPHA ALPHA gamma"),
        text(&view),
    );
    insert_pass(&view);
}

fn insert_pass(view: &EditorTextView) {
    let before = text(view);
    keys(view, "jofirst<Esc>");
    keys(view, "osecond<Esc>");
    let typed = text(view);
    keys(view, "uu");
    check(
        "each insert pass of several keys is one undo step",
        typed.contains("first\n") && typed.contains("second\n") && text(view) == before,
        format!("{typed:?} / {:?}", text(view)),
    );
    input_method(view);
}

fn input_method(view: &EditorTextView) {
    keys(view, "ggA");
    // SAFETY: Marked text and its replacement as an input method sends them, NSStrings both.
    unsafe {
        view.setMarkedText_selectedRange_replacementRange(
            &ns("にほ"),
            NSRange::new(2, 0),
            NSRange::new(objc2_foundation::NSNotFound.cast_unsigned(), 0),
        );
    }
    editor::type_text(view, "日本");
    keys(view, "<Esc>");
    let line = text(view).lines().next().unwrap_or_default().to_string();
    check(
        "an input method composes and types in Insert mode",
        line.ends_with("日本") && editor::vim_mode(view) == Some(ide_vim::Mode::Normal),
        line,
    );
    keys(view, "$^");
    let caret = view.selectedRange().location;
    check(
        "^ from a German keyboard's dead key goes to the line's first word",
        caret == 2,
        format!("caret at {caret}"),
    );
    control_keys(view);
}

fn control_keys(view: &EditorTextView) {
    let before = text(view);
    keys(view, "x");
    command(view, objc2::sel!(cancelOperation:));
    keys(view, "u");
    control(view, 'r');
    let redone = text(view);
    check("⌃R redoes in Normal mode", redone != before, format!("{redone:?}"));
    if let Some(controller) = window::for_project(&root()) {
        controller.open_file_beside(&root().join(OTHER), crate::window::panes::Side::Right);
    }
    let Some(main) = editor_of(FILE) else {
        return give_up(SCENARIO, "the editor is open");
    };
    wait_until(
        || editor_of(OTHER).is_some(),
        move |_| {
            if let Some(other) = editor_of(OTHER)
                && let Some(window) = other.window()
            {
                window.makeFirstResponder(Some(&other));
            }
            let kept = text(&main);
            control(&main, 'r');
            check(
                "⌃R stays with the editor that has the focus when another one is beside it",
                text(&main) == kept,
                "",
            );
            picture("vim-split");
            agent_change(&main);
        },
    );
}

fn agent_change(view: &EditorTextView) {
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(view));
    }
    let start = text(view);
    keys(view, "Otyped");
    let length = view.document().text().len_utf16();
    let change = view.document_mut().apply(ide_document::Edit {
        range_utf16: length..length,
        text: "agent\n".into(),
    });
    if let Ok(change) = change {
        view.apply_model_changes(&[change]);
    }
    keys(view, " more<Esc>");
    keys(view, "u");
    let after_one = text(view);
    check(
        "an agent's change during an insert pass is an undo step of its own",
        after_one.contains("agent\n") && !after_one.contains("typed more") && after_one != start,
        after_one,
    );
    editor::toggle_vim();
    let off = editor::vim_mode(view);
    check(
        "Editor ▸ Vim-Modus turns it off again",
        off.is_none(),
        format!("{off:?}"),
    );
    let _ = editor::save_all();
    finish(SCENARIO);
}
