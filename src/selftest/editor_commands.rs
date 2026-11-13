//! The `editor-commands` scenario, in a folder the user does not trust, so no language server runs: brackets and
//! quotes typed in pairs, Return and Tab indenting, commenting lines in and out per language and "Gehe zu Zeile" as the
//! keyboard and the menu bar send them; folds, extra carets and line wrapping from their commands; saving over a file
//! changed outside, a save that fails, closing a tab with unsaved changes; a compiler's line and column opened; a
//! workspace edit that renames an open file; and a server's question with its answers as a sheet.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_lsp::lsp_types::{
    DocumentChangeOperation, DocumentChanges, MessageActionItem, RenameFile, ResourceOp, WorkspaceEdit,
};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn, NSApplication, NSTextField, NSView,
};
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::helpers::{
    alert_texts, answer_alert, answer_alert_saying, in_any_mode_after, text_of, type_at, utf16_len, wait_until,
};
use super::{check, finish};
use crate::editor::{self, EditorTextView};
use crate::util::{main_after, ns};
use crate::window::{self, WindowController};

const SCENARIO: &str = "editor-commands";
/// Long enough for a command's effects to show and an alert to be up.
const SETTLE: Duration = Duration::from_millis(300);

const MAIN: &str = "fn main() {\n    let total = 1;\n\n    let other = total;\n}\n";
const FILES: [(&str, &str); 10] = [
    ("src/main.rs", MAIN),
    ("pairs.rs", "x\n"),
    ("brace.rs", "fn f() {}\n"),
    ("script.py", "def f():\n    return 1\n"),
    ("query.sql", "select 1;\n"),
    ("notes.md", "# Notes\n"),
    ("disk.txt", "first\n"),
    ("locked/file.txt", "kept\n"),
    ("moved.txt", "move me\n"),
    ("list.rs", "let x = a"),
];

fn folder() -> PathBuf {
    ide_core::paths::home().join("commands-project")
}

fn root() -> PathBuf {
    let folder = folder();
    folder.canonicalize().unwrap_or(folder)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

/// The editor of `file`, opened if it is not yet.
fn open(file: &str) -> Option<Retained<EditorTextView>> {
    let controller = controller()?;
    let path = root().join(file);
    controller.open_file(&path);
    editor::editor_for(&controller, &path)
}

/// Sends `action` to `view` as the menu bar or the keyboard does.
fn send(view: &EditorTextView, action: Sel) {
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it with no sender, as a key equivalent sends it.
    unsafe { app.sendAction_to_from(action, Some(view), None) };
}

fn selection(view: &EditorTextView) -> (usize, usize) {
    let range = view.selectedRange();
    (range.location, range.length)
}

fn line_start(view: &EditorTextView, line: usize) -> usize {
    view.line_start(line).unwrap_or(usize::MAX)
}

fn unit(view: &EditorTextView) -> String {
    view.ivars().indent.borrow().clone()
}

pub fn run() {
    let folder = folder();
    for (file, text) in FILES {
        let path = folder.join(file);
        let written =
            std::fs::create_dir_all(path.parent().unwrap_or(&folder)).and_then(|()| std::fs::write(&path, text));
        if let Err(error) = written {
            check("the files of the folder are made", false, error.to_string());
            return finish(SCENARIO);
        }
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, folder);
    let Some(window) = controller().and_then(|c| c.window()) else {
        check("the folder opens in a window", false, "");
        return finish(SCENARIO);
    };
    if let Some(sheet) = window.attachedSheet() {
        window.endSheet_returnCode(&sheet, NSAlertSecondButtonReturn);
    }
    wait_until(
        || open("pairs.rs").is_some(),
        |ok| {
            if ok {
                pairs();
            } else {
                check("the file opens in an editor", false, "");
                finish(SCENARIO);
            }
        },
    );
}

/// Brackets and quotes typed in pairs, a closing one typed over, and a selection wrapped.
fn pairs() {
    let Some(view) = open("pairs.rs") else {
        return finish(SCENARIO);
    };
    type_at(&view, NSRange::new(0, 0), "(");
    check(
        "a bracket typed in front of a word stays single",
        text_of(&view) == "(x\n",
        format!("{:?}", text_of(&view)),
    );
    type_at(&view, NSRange::new(2, 0), "(");
    let paired = (text_of(&view), selection(&view));
    check(
        "a bracket typed before a line's end gets its closing one, the caret between them",
        paired == ("(x()\n".to_string(), (3, 0)),
        format!("{paired:?}"),
    );
    type_at(&view, NSRange::new(3, 0), ")");
    let over = (text_of(&view), selection(&view));
    check(
        "typing the closing bracket steps over it",
        over == ("(x()\n".to_string(), (4, 0)),
        format!("{over:?}"),
    );
    type_at(&view, NSRange::new(4, 0), "a\"");
    check(
        "a quote right after a letter stays single",
        text_of(&view) == "(x()a\"\n",
        format!("{:?}", text_of(&view)),
    );
    type_at(&view, NSRange::new(1, 1), "[");
    let wrapped = (text_of(&view), selection(&view));
    check(
        "a bracket typed over a selection wraps it and keeps it selected",
        wrapped == ("([x]()a\"\n".to_string(), (2, 1)),
        format!("{wrapped:?}"),
    );
    main_after(SETTLE, returns);
}

/// Return between brackets and after a colon, and Tab over several lines.
fn returns() {
    let (Some(brace), Some(script)) = (open("brace.rs"), open("script.py")) else {
        return finish(SCENARIO);
    };
    brace.setSelectedRange(NSRange::new(8, 0));
    send(&brace, sel!(insertNewline:));
    let unit_rs = unit(&brace);
    let expected = format!("fn f() {{\n{unit_rs}\n}}\n");
    let split = (text_of(&brace), selection(&brace).0);
    check(
        "Return between braces puts the closing one on a line of its own and the caret indented between",
        split == (expected.clone(), 9 + unit_rs.len()),
        format!("{split:?}, wanted {expected:?}"),
    );
    let unit_py = unit(&script);
    script.setSelectedRange(NSRange::new(8, 0));
    send(&script, sel!(insertNewline:));
    let deeper = text_of(&script);
    check(
        "Return after a colon indents the new line one level deeper",
        deeper == format!("def f():\n{unit_py}\n    return 1\n"),
        format!("{deeper:?}"),
    );
    let all = utf16_len(&deeper);
    script.setSelectedRange(NSRange::new(0, all));
    send(&script, sel!(insertTab:));
    let indented = (text_of(&script), selection(&script));
    let wanted = format!("{unit_py}def f():\n{unit_py}\n{unit_py}    return 1\n");
    check(
        "Tab over several lines indents each of them but the empty one and keeps them selected",
        indented == (wanted.clone(), (0, utf16_len(&wanted))),
        format!("{indented:?}"),
    );
    main_after(SETTLE, comments);
}

/// Commenting lines out and in again, with the marker of each language.
fn comments() {
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    let end = line_start(&main, 4);
    main.setSelectedRange(NSRange::new(line_start(&main, 1), end - line_start(&main, 1)));
    send(&main, sel!(ideToggleComment:));
    let commented = text_of(&main);
    check(
        "the selected lines are commented out at their common indentation, the empty one left alone",
        commented == "fn main() {\n    // let total = 1;\n\n    // let other = total;\n}\n",
        format!("{commented:?}"),
    );
    send(&main, sel!(ideToggleComment:));
    check(
        "commenting lines that all are comments takes the markers out again",
        text_of(&main) == MAIN,
        format!("{:?}", text_of(&main)),
    );
    let marked = |file: &str| {
        open(file).map(|view| {
            view.setSelectedRange(NSRange::new(0, 0));
            send(&view, sel!(ideToggleComment:));
            text_of(&view)
        })
    };
    let python = marked("script.py").unwrap_or_default();
    let (sql, markdown) = (marked("query.sql"), marked("notes.md"));
    check(
        "Python comments with #, SQL with -- and Markdown has no line comment",
        python
            .split('\n')
            .next()
            .is_some_and(|first| first.trim_start() == "# def f():")
            && sql.as_deref() == Some("-- select 1;\n")
            && markdown.as_deref() == Some("# Notes\n"),
        format!("{python:?} {sql:?} {markdown:?}"),
    );
    main_after(SETTLE, || go_to_line(&["abc", "3", "999"], Vec::new()));
}

/// The editable field of the alert in front.
fn alert_field(view: &NSView) -> Option<Retained<NSTextField>> {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        return Some(field.retain());
    }
    view.subviews().iter().find_map(|child| alert_field(&child))
}

/// "Gehe zu Zeile" answered with each of `answers` in turn, each time from the cursor at the text's start.
fn go_to_line(answers: &'static [&'static str], mut reached: Vec<usize>) {
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    let Some((answer, rest)) = answers.split_first() else {
        let last = main.document().text().line_count().saturating_sub(1);
        let lines: Vec<usize> = [0, 2, last].iter().map(|line| line_start(&main, *line)).collect();
        check(
            "Gehe zu Zeile ignores what is no number, goes to line 3 and stops at the last line",
            reached == lines,
            format!("{reached:?}, lines start at {lines:?}"),
        );
        return main_after(SETTLE, cancelled_line);
    };
    main.setSelectedRange(NSRange::new(0, 0));
    in_any_mode_after(SETTLE, move || {
        let app = MainThreadMarker::new().map(NSApplication::sharedApplication);
        let field = app
            .and_then(|app| app.modalWindow())
            .and_then(|w| w.contentView())
            .and_then(|v| alert_field(&v));
        if let Some(field) = field {
            field.setStringValue(&ns(answer));
        }
        answer_alert(NSAlertFirstButtonReturn);
    });
    send(&main, sel!(ideGoToLine:));
    reached.push(selection(&main).0);
    main_after(SETTLE, move || go_to_line(rest, reached));
}

fn cancelled_line() {
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    main.setSelectedRange(NSRange::new(3, 0));
    let asked = answer_alert_saying(SETTLE, t("Gehe zu Zeile").to_string(), NSAlertSecondButtonReturn);
    send(&main, sel!(ideGoToLine:));
    check(
        "cancelling Gehe zu Zeile leaves the cursor",
        asked.get() && selection(&main) == (3, 0),
        format!("asked {}, {:?}", asked.get(), selection(&main)),
    );
    main_after(SETTLE, folds_and_carets);
}

/// Folding from the commands, extra carets above, below and at the next occurrence, and line wrapping.
fn folds_and_carets() {
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    use crate::editor::selftest::{carets, copied, folded, set_carets};
    send(&main, sel!(ideFoldAll:));
    let all = folded(&main);
    send(&main, sel!(ideUnfoldAll:));
    let none = folded(&main);
    main.setSelectedRange(NSRange::new(line_start(&main, 1) + 4, 0));
    send(&main, sel!(ideFold:));
    let one = folded(&main);
    send(&main, sel!(ideUnfold:));
    check(
        "the fold commands fold everything, nothing, and the block at the cursor",
        all >= 1 && none == 0 && one == 1 && folded(&main) == 0,
        format!("all {all}, none {none}, one {one}, after {}", folded(&main)),
    );
    main.setSelectedRange(NSRange::new(line_start(&main, 1) + 4, 0));
    send(&main, sel!(ideAddCaretBelow:));
    send(&main, sel!(ideAddCaretAbove:));
    let three = carets(&main).len();
    let total = line_start(&main, 1) + 8;
    set_carets(&main, std::iter::once(total..total).collect());
    main.setSelectedRange(NSRange::new(total, 5));
    send(&main, sel!(ideAddNextOccurrence:));
    let both = copied(&main);
    check(
        "carets are added below and above, and the next occurrence of the selected word is selected too",
        three == 3 && both.as_deref() == Some("total\ntotal"),
        format!("{three} carets, copied {both:?}"),
    );
    set_carets(&main, std::iter::once(0..0).collect());
    let before = editor::wraps(&main);
    send(&main, sel!(ideToggleWrap:));
    let toggled = editor::wraps(&main);
    send(&main, sel!(ideToggleWrap:));
    check(
        "Zeilen umbrechen switches wrapping and back",
        toggled != before && editor::wraps(&main) == before,
        format!("{before} → {toggled}"),
    );
    main_after(SETTLE, option_mouse);
}

/// A mouse event of `kind` with `flags` at UTF-16 offset `at` of `view`, `dx` points to its right.
fn mouse_at(
    view: &EditorTextView,
    kind: objc2_app_kit::NSEventType,
    flags: objc2_app_kit::NSEventModifierFlags,
    at: usize,
    dx: f64,
) -> Option<Retained<objc2_app_kit::NSEvent>> {
    let rect = crate::lsp::rect_of(view, at..at + 1)?;
    let point = objc2_foundation::NSPoint::new(rect.origin.x + dx + 1.0, rect.origin.y + rect.size.height / 2.0);
    objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,
        view.convertPoint_toView(point, None),
        flags,
        0.0,
        view.window()?.windowNumber(),
        None,
        0,
        1,
        0.0,
    )
}

/// ⌥-click adds a caret, ⌥-drag selects a column, a command other than a move or an edit leaves one caret, and text
/// of another number of lines than carets goes in whole at each.
fn option_mouse() {
    use objc2_app_kit::{NSEventModifierFlags as Flags, NSEventType as Kind};
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    use crate::editor::selftest::{carets, paste_text, set_carets};
    let app = NSApplication::sharedApplication(main.mtm());
    let (one, three) = (line_start(&main, 1) + 4, line_start(&main, 3) + 4);
    main.setSelectedRange(NSRange::new(one, 0));
    let click = |down: Option<Retained<objc2_app_kit::NSEvent>>, then: &[Option<Retained<objc2_app_kit::NSEvent>>]| {
        for event in then.iter().flatten() {
            app.postEvent_atStart(event, false);
        }
        if let Some(down) = down {
            // SAFETY: The view's handler of a mouse press, given one in its window, as AppKit sends it.
            let _: () = unsafe { objc2::msg_send![&*main, mouseDown: &*down] };
        }
    };
    click(
        mouse_at(&main, Kind::LeftMouseDown, Flags::Option, three, 0.0),
        &[mouse_at(&main, Kind::LeftMouseUp, Flags::Option, three, 0.0)],
    );
    let clicked = carets(&main);
    click(
        mouse_at(&main, Kind::LeftMouseDown, Flags::Option, one, 0.0),
        &[
            mouse_at(&main, Kind::LeftMouseDragged, Flags::Option, three, 24.0),
            mouse_at(&main, Kind::LeftMouseUp, Flags::Option, three, 24.0),
        ],
    );
    let block = carets(&main);
    paste_text(&main, "ab");
    let pasted = text_of(&main);
    press(&main, sel!(scrollPageDown:));
    let left = carets(&main).len();
    check(
        "⌥-click adds a caret and ⌥-drag selects a block, one range per line, empty on the empty line",
        clicked.len() == 2
            && clicked.contains(&(three..three))
            && block.len() == 3
            && block.iter().any(Range::is_empty),
        format!("{clicked:?} {block:?}"),
    );
    check(
        "text of one line goes in at every caret of the block, and a page down leaves one caret",
        pasted.matches("ab").count() == 3 && left == 1,
        format!("{pasted:?}, {left} left"),
    );
    if let Some(undo) = main.undoManager() {
        undo.undo();
    }
    set_carets(&main, std::iter::once(0..0).collect());
    main_after(SETTLE, gutter);
}

/// Clicks in the line number column: the fold strip folds and unfolds, a number sets and clears a breakpoint, the
/// change strip of an unchanged line opens nothing, a right click offers the breakpoint's menu, and the column
/// follows the mouse entering and leaving, for the open folds' triangles.
fn gutter() {
    use objc2_app_kit::NSEventType;
    let (Some(controller), Some(main)) = (controller(), open("src/main.rs")) else {
        return finish(SCENARIO);
    };
    let path = root().join("src/main.rs");
    let click = |from_right: f64, kind: NSEventType| editor::gutter_mouse(&main, 0, from_right, kind);
    let fold_strip = 8.0 + 6.0;
    click(fold_strip, NSEventType::LeftMouseDown);
    let folded = editor::selftest::folded(&main);
    click(fold_strip, NSEventType::LeftMouseDown);
    check(
        "a click on the fold strip folds the block and a second one unfolds it",
        folded == 1 && editor::selftest::folded(&main) == 0,
        format!("{folded} then {}", editor::selftest::folded(&main)),
    );
    let number = 8.0 + 12.0 + 8.0;
    click(number, NSEventType::LeftMouseDown);
    let set = crate::debug::selftest::breakpoint_at(&controller, &path, 0).is_some();
    click(number, NSEventType::LeftMouseDown);
    let cleared = crate::debug::selftest::breakpoint_at(&controller, &path, 0).is_none();
    check(
        "a click on a line number sets a breakpoint there and a second one clears it",
        set && cleared,
        format!("set {set}, cleared {cleared}"),
    );
    click(1.0, NSEventType::LeftMouseDown);
    let popover = editor::change_popover(&main).is_some();
    let menu = click(number, NSEventType::RightMouseDown)
        .map(|(titles, _)| titles)
        .unwrap_or_default();
    check(
        "the change strip of an unchanged line opens nothing, and a right click offers to set a breakpoint",
        !popover && menu.first().map(String::as_str) == Some(t("Haltepunkt setzen")),
        format!("popover {popover}, menu {menu:?}"),
    );
    let entered = click(number, NSEventType::MouseEntered).is_some_and(|(_, over)| over);
    let left = click(number, NSEventType::MouseExited).is_some_and(|(_, over)| !over);
    check(
        "the column follows the mouse entering and leaving it, for the open folds' triangles",
        entered && left,
        format!("{entered} {left}"),
    );
    main_after(SETTLE, indentation_menu);
}

/// The status bar's indentation menu ticks what the file uses, and its entries switch to tabs and another width.
fn indentation_menu() {
    let Some(script) = open("script.py") else {
        return finish(SCENARIO);
    };
    let menu = editor::indent_menu(&script);
    let ticked: Vec<String> = menu
        .itemArray()
        .iter()
        .filter(|item| item.state() == objc2_app_kit::NSControlStateValueOn)
        .map(|item| item.title().to_string())
        .collect();
    let choose = |title: String| {
        let item = menu.itemArray().iter().find(|item| item.title().to_string() == title);
        if let Some(item) = item {
            let app = NSApplication::sharedApplication(script.mtm());
            // SAFETY: The entry's own action, sent to the editor with the entry as the sender, as the menu sends it.
            unsafe { app.sendAction_to_from(sel!(ideSetIndent:), Some(&script), Some(&item)) };
        }
    };
    choose(t("Mit Tabs einrücken").to_string());
    choose(tf("Breite: {}", &["2"]));
    let shown = editor::indent_shown(&script).0;
    check(
        "the indentation menu ticks spaces and their width, and its entries switch to tabs two columns wide",
        ticked == [t("Mit Leerzeichen einrücken").to_string(), tf("Breite: {}", &["4"])]
            && shown == tf("Tabs: {}", &["2"]),
        format!("{ticked:?} → {shown}"),
    );
    main_after(SETTLE, completion_keys);
}

/// Presses `command` in `view` as the keyboard does.
fn press(view: &EditorTextView, command: Sel) {
    use objc2_app_kit::NSStandardKeyBindingResponding;
    let text_view: &objc2_app_kit::NSTextView = view;
    // SAFETY: A selector of the standard key bindings, which the text view answers, as a key press sends it.
    unsafe { text_view.doCommandBySelector(command) };
}

/// The keyboard in the completion list, opened as a server's answer would: arrows choose, Return takes, typing
/// filters, deleting widens again, Esc and other keys close it; and Tab through a snippet's stops.
fn completion_keys() {
    use crate::lsp::completion;
    let Some(view) = open("list.rs") else {
        return finish(SCENARIO);
    };
    const LABELS: [&str; 5] = ["alpha", "beta", "abc", "able", "array"];
    view.setSelectedRange(NSRange::new(9, 0));
    completion::show(&view, &LABELS);
    let ranked = completion::shown(&view);
    press(&view, sel!(moveDown:));
    press(&view, sel!(moveDown:));
    press(&view, sel!(moveUp:));
    press(&view, sel!(insertNewline:));
    check(
        "the list ranks prefixes first, the arrows choose and Return takes the chosen item",
        ranked == ["abc", "able", "alpha", "array", "beta"] && text_of(&view) == "let x = able",
        format!("{ranked:?} → {:?}", text_of(&view)),
    );
    type_at(&view, NSRange::new(12, 0), " a");
    completion::show(&view, &LABELS);
    type_at(&view, NSRange::new(14, 0), "b");
    let narrowed = completion::shown(&view);
    press(&view, sel!(deleteBackward:));
    let widened = completion::shown(&view).len();
    press(&view, sel!(pageDown:));
    press(&view, sel!(pageUp:));
    press(&view, sel!(cancelOperation:));
    let closed = completion::shown(&view).is_empty();
    check(
        "typing narrows the list, deleting widens it again, and Esc closes it",
        narrowed == ["abc", "able"] && widened == 5 && closed,
        format!("{narrowed:?}, {widened}, closed {closed}"),
    );
    completion::show(&view, &LABELS);
    press(&view, sel!(moveLeft:));
    let left = (completion::shown(&view).is_empty(), selection(&view));
    completion::show(&view, &LABELS);
    type_at(&view, NSRange::new(13, 0), "-");
    check(
        "another key closes the list and does what it does, and so does a character that is no part of a word",
        left == (true, (13, 0)) && completion::shown(&view).is_empty(),
        format!("{left:?}"),
    );
    completion::snippet(&view, std::iter::once(4..5).collect());
    press(&view, sel!(insertTab:));
    let stop = selection(&view);
    completion::snippet(&view, Vec::new());
    let before = utf16_len(&text_of(&view));
    view.setSelectedRange(NSRange::new(before, 0));
    press(&view, sel!(insertTab:));
    let tabbed = utf16_len(&text_of(&view)) - before;
    completion::snippet(&view, std::iter::once(0..1).collect());
    press(&view, sel!(cancelOperation:));
    press(&view, sel!(insertTab:));
    check(
        "Tab goes to a snippet's next stop, past the last one it indents again, and Esc ends the snippet",
        stop == (4, 1) && tabbed == unit(&view).len() && utf16_len(&text_of(&view)) == before + 2 * tabbed,
        format!("{stop:?}, tab {tabbed}"),
    );
    main_after(SETTLE, changed_outside);
}

/// Saving over a file that changed on disk asks first; a save that fails says so.
fn changed_outside() {
    let Some(disk) = open("disk.txt") else {
        return finish(SCENARIO);
    };
    type_at(&disk, NSRange::new(0, 0), "mine ");
    let _ = std::fs::write(root().join("disk.txt"), "theirs\n");
    let asked = answer_alert_saying(
        SETTLE,
        tf("„{}“ wurde inzwischen außerhalb geändert.", &["disk.txt"]),
        NSAlertFirstButtonReturn,
    );
    send(&disk, sel!(ideSave:));
    let written = std::fs::read_to_string(root().join("disk.txt")).unwrap_or_default();
    check(
        "saving over a file changed outside asks, and Überschreiben writes the editor's text",
        asked.get() && written == "mine first\n" && !disk.document().is_dirty(),
        format!("asked {}, on disk {written:?}", asked.get()),
    );
    main_after(SETTLE, failed_save);
}

fn set_mode(path: &Path, mode: u32) {
    let _ = std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode));
}

fn failed_save() {
    let Some(locked) = open("locked/file.txt") else {
        return finish(SCENARIO);
    };
    type_at(&locked, NSRange::new(0, 0), "new ");
    let (dir, file) = (root().join("locked"), root().join("locked/file.txt"));
    set_mode(&file, 0o444);
    set_mode(&dir, 0o555);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let texts = seen.clone();
    in_any_mode_after(SETTLE, move || {
        texts.borrow_mut().extend(alert_texts());
        answer_alert(NSAlertFirstButtonReturn);
    });
    send(&locked, sel!(ideSave:));
    set_mode(&dir, 0o755);
    set_mode(&file, 0o644);
    let said = seen.borrow().iter().any(|text| text == t("Sichern fehlgeschlagen"));
    check(
        "a save the folder refuses says so and keeps the changes unsaved",
        said && locked.document().is_dirty(),
        format!("{:?}", seen.borrow()),
    );
    send(&locked, sel!(ideSave:));
    check(
        "once the folder may be written again the file saves",
        !locked.document().is_dirty(),
        "",
    );
    main_after(SETTLE, close_dirty);
}

/// Closing a tab with unsaved changes asks; Abbrechen keeps it, Nicht sichern closes it and leaves the file.
fn close_dirty() {
    let (Some(controller), Some(notes)) = (controller(), open("notes.md")) else {
        return finish(SCENARIO);
    };
    type_at(&notes, NSRange::new(0, 0), "x");
    let Some(tab) = editor::editors_of(&controller)
        .into_iter()
        .find_map(|(tab, view)| std::ptr::eq(&*view, &*notes).then_some(tab))
    else {
        return finish(SCENARIO);
    };
    let question = tf("Änderungen an „{}“ sichern?", &["notes.md"]);
    let kept = answer_alert_saying(SETTLE, question.clone(), NSAlertThirdButtonReturn);
    controller.close_tab(tab);
    let still = controller.tab_ids().contains(&tab);
    let gone = answer_alert_saying(SETTLE, question, NSAlertSecondButtonReturn);
    controller.close_tab(tab);
    let disk = std::fs::read_to_string(root().join("notes.md")).unwrap_or_default();
    check(
        "closing a changed tab asks: Abbrechen keeps it, Nicht sichern closes it without writing",
        kept.get() && still && gone.get() && !controller.tab_ids().contains(&tab) && disk == "# Notes\n",
        format!("kept {} {still}, gone {}, disk {disk:?}", kept.get(), gone.get()),
    );
    main_after(SETTLE, positions);
}

/// A compiler's line and column open the file with the cursor there; the files below the folder save at once; a file
/// changed by the app's own git is taken.
fn positions() {
    let Some(controller) = controller() else {
        return finish(SCENARIO);
    };
    let main_path = root().join("src/main.rs");
    editor::reveal_position(&controller, &main_path, 2, 9);
    let Some(main) = open("src/main.rs") else {
        return finish(SCENARIO);
    };
    let at = selection(&main).0;
    editor::reveal_position(&controller, &main_path, 2, 999);
    let end = selection(&main).0;
    check(
        "line and column from a compiler put the cursor there, a column past the end at the line's end",
        at == line_start(&main, 1) + 8 && end == line_start(&main, 2) - 1,
        format!("{at} {end}"),
    );
    let unsaved = editor::unsaved_below(&root());
    let saved = editor::save_below(&root());
    let names: Vec<String> = unsaved.iter().map(|p| editor::display_name(p)).collect();
    check(
        "the changed files below the folder are listed and saved",
        names.contains(&"pairs.rs".to_string()) && names.contains(&"script.py".to_string()) && saved,
        format!("{names:?}, saved {saved}"),
    );
    let _ = std::fs::write(&main_path, "fn main() {}\n");
    editor::check_disk_below(&root());
    check(
        "after the app's own git changed a file, its unchanged editor takes the new text",
        text_of(&main) == "fn main() {}\n",
        format!("{:?}", text_of(&main)),
    );
    main_after(SETTLE, file_operations);
}

/// A workspace edit renames an open file, and its editor and tab follow.
fn file_operations() {
    if open("moved.txt").is_none() {
        return finish(SCENARIO);
    }
    let uri = |file: &str| ide_lsp::uri::from_path(&root().join(file));
    let edit = WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Operations(vec![DocumentChangeOperation::Op(
            ResourceOp::Rename(RenameFile {
                old_uri: uri("moved.txt"),
                new_uri: uri("renamed.txt"),
                options: None,
                annotation_id: None,
            }),
        )])),
        change_annotations: None,
    };
    let applied = crate::lsp::apply_workspace_edit(&root(), &edit);
    let titles = controller().map(|c| c.tab_titles()).unwrap_or_default();
    let renamed = root().join("renamed.txt");
    let follows = controller().and_then(|c| editor::editor_for(&c, &renamed)).is_some();
    check(
        "a workspace edit renames an open file, and its editor and tab follow",
        applied.is_ok() && follows && titles.contains(&"renamed.txt".to_string()) && !root().join("moved.txt").exists(),
        format!("{applied:?}, tabs {titles:?}"),
    );
    main_after(SETTLE, server_question);
}

/// A server's question shows its answers as buttons of a sheet; the chosen one is answered.
fn server_question() {
    let Some(window) = controller().and_then(|c| c.window()) else {
        return finish(SCENARIO);
    };
    let chosen: Rc<Cell<Option<Option<String>>>> = Rc::new(Cell::new(None));
    let heard = chosen.clone();
    let actions = ["Retry", "Ignore", "Report", "More"].map(|title| MessageActionItem {
        title: title.to_string(),
        properties: Default::default(),
    });
    crate::lsp::server_question(&root(), "The build failed.", actions.to_vec(), move |answer| {
        heard.set(Some(answer.map(|a| a.title)));
    });
    let buttons: Vec<String> = window
        .attachedSheet()
        .and_then(|sheet| sheet.contentView())
        .map(|content| button_titles(&content))
        .unwrap_or_default();
    if let Some(sheet) = window.attachedSheet() {
        window.endSheet_returnCode(&sheet, NSAlertSecondButtonReturn);
    }
    in_any_mode_after(SETTLE, move || {
        let answer = chosen.take();
        check(
            "a server's question shows three of its answers and a close button, and the chosen one is answered",
            buttons == ["Retry", "Ignore", "Report", t("Schließen")] && answer == Some(Some("Ignore".to_string())),
            format!("{buttons:?} → {answer:?}"),
        );
        done();
    });
}

fn button_titles(view: &NSView) -> Vec<String> {
    let mut titles = Vec::new();
    if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>() {
        titles.push(button.title().to_string());
    }
    for child in view.subviews().iter() {
        titles.extend(button_titles(&child));
    }
    titles
}

fn done() {
    let saved = editor::save_all();
    check(
        "every editor is saved before quitting",
        saved,
        format!("{} unsaved", editor::dirty_count()),
    );
    finish(SCENARIO);
}
