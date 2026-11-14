//! The `editor-multi` scenario, without a network or a language server: several carets typing, deleting, pairing
//! brackets, indenting and pasting as one undoable step each, the next occurrence, a column block, Esc; then folds
//! by indentation hiding their lines, unfolding by an edit, by the placeholder and all at once.

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::NSStandardKeyBindingResponding;
use objc2_foundation::{NSPoint, NSRange};

use super::{check, finish, helpers, snapshot};
use crate::editor::{self, EditorTextView, selftest as edit};
use crate::window::{self, WindowController};

const SCENARIO: &str = "editor-multi";
const PROJECT: &str = "multi-project";
/// A turn of the event loop between steps, so that AppKit closes the undo group of one step's typing before the next.
const EVENT_TURN: Duration = Duration::from_millis(100);

const LINES: &str = "alpha\nbeta\ngamma\n";
const BLOCKS: &str = "if a {}\nif b {}\n";
const WORDS: &str = "foo bar foo baz foo\n";
const PYTHON: &str = "def one():\n    a = 1\n    b = 2\n\ndef two():\n    c = 3\n";
const NOTE: &str = "one\n";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn project_window() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn open(file: &str) -> Option<Retained<EditorTextView>> {
    let controller = project_window()?;
    let path = root().join(file);
    controller.open_file(&path);
    editor::editor_for(&controller, &path)
}

fn text(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

fn command(view: &EditorTextView, command: objc2::runtime::Sel) {
    // SAFETY: A command of the standard key bindings, which the text view answers.
    unsafe { view.doCommandBySelector(command) };
}

fn undo_redo(view: &EditorTextView) -> String {
    let Some(undo) = view.undoManager() else {
        return String::new();
    };
    undo.undo();
    let undone = text(view);
    undo.redo();
    undone
}

fn type_str(view: &EditorTextView, s: &str) {
    for c in s.chars() {
        editor::type_text(view, &c.to_string());
    }
}

fn picture(name: &str) {
    if let Some(window) = project_window().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

pub fn run() {
    let files = [
        ("lines.txt", LINES),
        ("blocks.txt", BLOCKS),
        ("words.txt", WORDS),
        ("fold.py", PYTHON),
        ("note.txt", NOTE),
    ];
    helpers::open_project(SCENARIO, PROJECT, &files, |_| {
        helpers::wait_until(|| open("lines.txt").is_some(), |_| carets_type_and_delete());
    });
}

fn carets_type_and_delete() {
    let Some(view) = open("lines.txt") else {
        check("the file opens", false, "");
        finish(SCENARIO);
        return;
    };
    view.setSelectedRange(NSRange::new(0, 0));
    edit::add_vertical(&view, false);
    edit::add_vertical(&view, false);
    let carets = edit::carets(&view);
    check(
        "⌃⇧↓ adds carets below at the same column",
        carets == [0..0, 6..6, 11..11],
        format!("{carets:?}"),
    );
    type_str(&view, "x");
    let typed = edit::carets(&view);
    check(
        "typing goes to every caret",
        text(&view) == "xalpha\nxbeta\nxgamma\n",
        text(&view),
    );
    check("one undo takes it back everywhere", undo_redo(&view) == LINES, "");
    edit::set_carets(&view, typed);
    command(&view, sel!(deleteBackward:));
    check(
        "deleting backwards works at every caret",
        text(&view) == LINES,
        format!("{:?} {:?}", text(&view), edit::carets(&view)),
    );
    edit::paste_text(&view, "1\n2\n3\n");
    check(
        "pasting as many lines as carets gives each caret its line",
        text(&view) == "1alpha\n2beta\n3gamma\n",
        text(&view),
    );
    check("the paste is one undo step", undo_redo(&view) == LINES, "");
    edit::set_carets(&view, vec![1..3, 8..10]);
    check(
        "the selections are copied one per line",
        edit::copied(&view).as_deref() == Some("al\nbe"),
        edit::copied(&view).unwrap_or_default(),
    );
    picture("editor-multi-carets");
    command(&view, sel!(cancelOperation:));
    check(
        "Esc leaves one caret",
        edit::carets(&view).len() == 1,
        format!("{:?}", edit::carets(&view)),
    );
    helpers::after(EVENT_TURN, pairs_and_indent);
}

fn pairs_and_indent() {
    let Some(view) = open("blocks.txt") else { return };
    edit::set_carets(&view, vec![6..6, 14..14]);
    command(&view, sel!(insertNewline:));
    check(
        "a new line between braces is indented at every caret",
        text(&view) == "if a {\n    \n}\nif b {\n    \n}\n",
        text(&view),
    );
    type_str(&view, "(");
    check(
        "brackets close at every caret",
        text(&view) == "if a {\n    ()\n}\nif b {\n    ()\n}\n",
        text(&view).replace('\n', "|"),
    );
    let Some(lines) = open("lines.txt") else { return };
    let left = column_point(&lines, 1);
    let right = column_point(&lines, 16);
    if let (Some(left), Some(right)) = (left, right) {
        edit::column(&lines, left, right);
    }
    let block = edit::carets(&lines);
    type_str(&lines, "Z");
    check(
        "a column block selects the same columns on each line, and typing replaces them",
        block == [1..3, 8..10, 14..16] && text(&lines) == "1Zpha\n2Zta\n3Zmma\n",
        format!("{block:?} → {}", text(&lines).replace('\n', "|")),
    );
    let Some(words) = open("words.txt") else { return };
    words.setSelectedRange(NSRange::new(0, 3));
    edit::add_next_occurrence(&words);
    edit::add_next_occurrence(&words);
    let found = edit::carets(&words);
    type_str(&words, "qux");
    check(
        "⌘D adds the next occurrences, and typing replaces each",
        found == [0..3, 8..11, 16..19] && text(&words) == "qux bar qux baz qux\n",
        format!("{found:?} → {:?} → {:?}", text(&words), edit::carets(&words)),
    );
    helpers::after(EVENT_TURN, folds);
}

/// The point of the given offset's caret in `view`, at the middle of its line.
fn column_point(view: &EditorTextView, offset: usize) -> Option<NSPoint> {
    let rect = crate::lsp::rect_of(view, offset..offset)?;
    Some(NSPoint::new(rect.origin.x, rect.origin.y + rect.size.height / 2.0))
}

fn folds() {
    let Some(view) = open("fold.py") else { return };
    view.setSelectedRange(NSRange::new(3, 0));
    edit::fold_at_cursor(&view);
    let shown = edit::laid_out_lines(&view);
    check(
        "folding by indentation hides the block's lines",
        edit::folded(&view) == 1 && !shown.contains(&1) && !shown.contains(&2) && shown.contains(&3),
        format!("{shown:?}"),
    );
    check(
        "the folded line shows a triangle and a placeholder",
        edit::gutter(&view).get(&0) == Some(&true) && edit::placeholders(&view).len() == 1,
        format!("{:?}", edit::gutter(&view)),
    );
    picture("editor-multi-folded");
    let Some((rect, _)) = edit::placeholders(&view).first().cloned() else {
        return;
    };
    let center = NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    check(
        "a click on the placeholder unfolds",
        edit::click_fold(&view, center) && edit::folded(&view) == 0,
        "",
    );
    edit::fold_all(&view);
    check("fold all folds both blocks", edit::folded(&view) == 2, "");
    let inside = text(&view)
        .find("c = 3")
        .map(|b| text(&view)[..b].encode_utf16().count());
    if let Some(inside) = inside {
        crate::lsp::apply_text_edits(&*view, vec![(inside..inside, "x".into())]);
    }
    helpers::wait_until(
        || open("fold.py").is_some_and(|view| edit::folded(&view) == 1),
        |_| unfolded_by_edit(),
    );
}

fn unfolded_by_edit() {
    let Some(view) = open("fold.py") else { return };
    let shown = edit::laid_out_lines(&view);
    check(
        "an edit inside a fold unfolds it",
        edit::folded(&view) == 1 && shown.contains(&5),
        format!("{shown:?}"),
    );
    edit::unfold_all(&view);
    let shown = edit::laid_out_lines(&view);
    check(
        "unfold all shows every line",
        (0..6).all(|line| shown.contains(&line)),
        format!("{shown:?}"),
    );
    helpers::after(EVENT_TURN, edit_after_typing);
}

/// A server's edit in the same turn of the run loop as the typing before it, while AppKit's group for the turn is
/// still open.
fn edit_after_typing() {
    let Some(view) = open("note.txt") else { return };
    view.setSelectedRange(NSRange::new(3, 0));
    editor::type_text(&view, "!");
    let open = view.undoManager().map(|undo| undo.groupingLevel());
    crate::lsp::apply_text_edits(&*view, vec![(0..0, "// ".into())]);
    let edited = text(&view);
    let first = view.undoManager().map(|undo| {
        undo.undo();
        let after_one = text(&view);
        undo.undo();
        (after_one, text(&view))
    });
    check(
        "an edit right after typing is an undo step of its own",
        open == Some(1) && edited == "// one!\n" && first == Some(("one!\n".into(), NOTE.into())),
        format!("group level after typing {open:?}; {edited:?} → {first:?}"),
    );
    editor::save_all();
    finish(SCENARIO);
}
