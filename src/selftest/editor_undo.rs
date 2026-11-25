//! The `editor-undo` scenario (E96): a fixed series of pseudo-random replacements in an editor, over text with
//! characters outside the BMP and all three line breaks, each one undo step; then undoing walks back through every
//! text in reverse and redoing forward to the last, with the document always holding what the view shows.

use std::time::Duration;

use objc2::rc::Retained;
use objc2_app_kit::NSTextView;
use objc2_foundation::NSRange;

use super::{check, finish, give_up, helpers};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "editor-undo";
const FILE: &str = "undo.txt";
const START: &str = "fn main() {\r\n    let ä = \"😀\";\n}\rtail\n";
const STEPS: usize = 60;
/// What the replacements are made of; no brackets or quotes, which the editor would pair.
const PIECES: [&str; 8] = ["a", "xyz", "ä", "😀", "e\u{301}", "\n", "\r\n", " "];

const PROJECT: &str = "undo-project";
/// A pause after the turn that made an edit, before the next one.
const EVENT_TURN: Duration = Duration::from_millis(20);

fn open() -> Option<Retained<EditorTextView>> {
    let root = helpers::root(PROJECT);
    let controller = window::for_project(&root)?;
    editor::editor_for(&controller, &root.join(FILE))
}

/// xorshift64, so that every run makes the same edits.
struct Random(u64);

impl Random {
    fn next(&mut self, below: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(below.max(1)).unwrap_or(1)).unwrap_or(0)
    }
}

/// The UTF-16 offsets of the character boundaries of `text`.
fn boundaries(text: &str) -> Vec<usize> {
    let mut out = vec![0];
    let mut utf16 = 0;
    for c in text.chars() {
        utf16 += c.len_utf16();
        out.push(utf16);
    }
    out
}

fn shown(view: &EditorTextView) -> String {
    let view: &NSTextView = view;
    view.string().to_string()
}

fn held(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

pub(crate) fn run() {
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, START)], |_| {
        let root = helpers::root(PROJECT);
        helpers::wait_for_editor(&root, &root.join(FILE), |view| {
            let Some(view) = view else {
                return give_up(SCENARIO, "the file opens");
            };
            let texts = vec![held(&view)];
            // Opening and the first edit in one turn of the event loop would share an undo group.
            helpers::after(EVENT_TURN, move || step(Random(0x00da_6e0d_1234_5678), texts, 0));
        });
    });
}

/// One replacement, then the next once an event has passed: AppKit closes an undo group only after the event loop
/// handled an event, so edits without one between them would undo as one.
fn step(random: Random, texts: Vec<String>, done: usize) {
    let Some(view) = open() else {
        return give_up(SCENARIO, "the file stays open");
    };
    if done < STEPS {
        return replace(&view, random, texts, done);
    }
    walk(&view, &texts);
}

fn replace(view: &EditorTextView, mut random: Random, mut texts: Vec<String>, done: usize) {
    let before = shown(view);
    let places = boundaries(&before);
    let (a, b) = (places[random.next(places.len())], places[random.next(places.len())]);
    let range = a.min(b)..a.max(b);
    let new: String = (0..random.next(4)).map(|_| PIECES[random.next(PIECES.len())]).collect();
    if !(range.is_empty() && new.is_empty()) {
        view.setSelectedRange(NSRange::new(range.start, range.len()));
        editor::type_text(view, &new);
        // Typing on where the last edit ended would continue its undo step, as AppKit means it to.
        let text_view: &NSTextView = view;
        text_view.breakUndoCoalescing();
        super::helpers::post_event();
        texts.push(shown(view));
    }
    // The next edit waits until AppKit closed the undo group of this one, which it does once the posted event passed.
    helpers::wait_until(
        || open().is_none_or(|view| view.undoManager().is_none_or(|undo| undo.groupingLevel() == 0)),
        move |_| helpers::after(EVENT_TURN, move || step(random, texts, done + 1)),
    );
}

fn walk(view: &EditorTextView, texts: &[String]) {
    check(
        "after every edit the document holds what the view shows",
        shown(view) == held(view) && texts.len() > STEPS / 2,
        format!("{} edits", texts.len() - 1),
    );
    let Some(undo) = view.undoManager() else {
        return give_up(SCENARIO, "the editor has an undo manager");
    };
    let mut back = Vec::new();
    for _ in 1..texts.len() {
        undo.undo();
        back.push(held(view));
    }
    let expected: Vec<String> = texts.iter().rev().skip(1).cloned().collect();
    let first_wrong = back.iter().zip(&expected).position(|(got, want)| got != want);
    check(
        "undo walks back through every text, one edit a step, to the file as it opened",
        first_wrong.is_none() && back.last().map(String::as_str) == Some(START) && shown(view) == START,
        match first_wrong {
            None => format!("{} steps", back.len()),
            Some(i) => format!(
                "step {i} gives {:?} instead of {:?}; the steps land on texts {:?} of {}",
                back.get(i),
                expected.get(i),
                back.iter()
                    .map(|got| texts.iter().rposition(|t| t == got))
                    .collect::<Vec<_>>(),
                texts.len()
            ),
        },
    );
    for _ in 1..texts.len() {
        undo.redo();
    }
    check(
        "redo comes forward to the last text",
        Some(&held(view)) == texts.last() && shown(view) == held(view),
        format!("{:?}", held(view)),
    );
    // Back to the file's text, so that quitting asks nothing.
    while undo.canUndo() {
        undo.undo();
    }
    finish(SCENARIO);
}
