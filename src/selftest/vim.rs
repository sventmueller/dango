//! The `editor-vim` scenario (B26): Editor ▸ Vim-Modus puts the editor in Normal mode with a block caret and the mode in
//! the status bar; keys through the input context run motions and operators, `ciw…Esc` and `.` repeat, `u` undoes on
//! the editor's history and an insert pass of several lines is one undo step; a Japanese input method types in Insert
//! mode; `^` of a German keyboard goes to the line's first word; a Cyrillic letter is the US key at its place only
//! on a keyboard that does not type ASCII; ⌃R redoes only in the editor with the focus, also with
//! a second editor beside it; and an agent's change during an insert pass is an undo step of its own. Then the
//! prompt and `:s` through the input context, the marked match, keys that wait for a character, insert in a Visual
//! block, `gv`, `=`, a paste after a visual delete and undo, and `:q`, `:wq`, `:x` and `:q!` closing only their own
//! document.

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
const KEEP: &str = "keep.txt";
const TEXT: &str = "  alpha beta gamma\nsecond line\nthird line\n";
/// Four spaces a level, as `=` is checked with.
const EDITORCONFIG: &str = "root = true\n\n[*]\nindent_style = space\nindent_size = 4\n";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn editor_of(file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(file))
}

fn text(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

/// Types `keys` as the input context hands them over: printable characters as text, `<Esc>`, `<Enter>` and `<BS>` as
/// their commands, and `<C-v>` as a key with ⌃.
fn keys(view: &EditorTextView, keys: &str) {
    let mut rest = keys;
    while let Some(c) = rest.chars().next() {
        if c == '<'
            && let Some(end) = rest.find('>')
            && named_key(view, &rest[1..end])
        {
            rest = &rest[end + 1..];
            continue;
        }
        editor::type_text(view, &c.to_string());
        rest = &rest[c.len_utf8()..];
    }
}

/// Presses the key `name`, such as `Esc` or `C-v`; whether there is one by that name.
fn named_key(view: &EditorTextView, name: &str) -> bool {
    let selector = match name {
        "Esc" => objc2::sel!(cancelOperation:),
        "Enter" => objc2::sel!(insertNewline:),
        "BS" => objc2::sel!(deleteBackward:),
        _ => {
            let Some(key) = name.strip_prefix("C-").and_then(|k| k.chars().next()) else {
                return false;
            };
            control(view, key);
            return true;
        }
    };
    command(view, selector);
    true
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
    let files = [
        (FILE, TEXT),
        (OTHER, "other\n"),
        (KEEP, "keep\n"),
        (".editorconfig", EDITORCONFIG),
    ];
    helpers::open_project(SCENARIO, PROJECT, &files, |controller| {
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
    cyrillic_key(view);
    control_keys(view);
}

/// A Cyrillic `о`, at the place of `j` on the US layout, moves down only where the keyboard does not type ASCII.
fn cyrillic_key(view: &EditorTextView) {
    let line_end = text(view).find('\n').unwrap_or_default();
    let line_end = text(view)[..line_end].encode_utf16().count();
    keys(view, "о");
    let caret = view.selectedRange().location;
    let latin = editor::vim_types_ascii();
    check(
        "a Cyrillic letter in Normal mode is the US key at its place only on a keyboard that does not type ASCII",
        (caret > line_end) != latin && editor::vim_mode(view) == Some(ide_vim::Mode::Normal),
        format!("caret at {caret}, first line ends at {line_end}, ASCII keyboard {latin}"),
    );
    keys(view, "<Esc>");
    if caret > line_end {
        keys(view, "k");
    }
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
    prompt(view);
}

/// Puts `new` in `view` as a change from outside, which Vim follows, with Vim's cursor at its start.
fn set_text(view: &EditorTextView, new: &str) {
    let length = view.document().text().len_utf16();
    let change = view.document_mut().apply(ide_document::Edit {
        range_utf16: 0..length,
        text: new.into(),
    });
    if let Ok(change) = change {
        view.apply_model_changes(&[change]);
    }
    keys(view, "<Esc>gg0");
}

fn label(view: &EditorTextView) -> String {
    editor::vim_shown(view).0
}

fn note(view: &EditorTextView) -> String {
    ide_ui_lsp::status_note(view).unwrap_or_default()
}

/// What `keys` make of `before`, and that one `u` takes it back.
fn one_undo_step(view: &EditorTextView, before: &str, typed: &str) -> (String, bool) {
    set_text(view, before);
    keys(view, typed);
    let after = text(view);
    keys(view, "<Esc>u");
    (after, text(view) == before)
}

fn prompt(view: &EditorTextView) {
    set_text(view, "x x\ny x\n");
    keys(view, ":/;");
    check(
        "/ and ; reach the prompt as typed, not as the keys of another layout",
        label(view) == ":/;",
        label(view),
    );
    keys(view, "<Esc>");
    let (whole, undone) = one_undo_step(view, "x x\ny x\n", ":%s/x/z/g<Enter>");
    check(
        ":%s/x/z/g typed through the input context changes every match, and one u takes it back",
        whole == "z z\ny z\n" && undone,
        format!("{whole:?}, undone {undone}"),
    );
    let (first, undone) = one_undo_step(view, "x x\ny x\n", ":s#x#z#<Enter>");
    check(
        ":s with # as separator changes the editor's text, and one u takes it back",
        first == "z x\ny x\n" && undone,
        format!("{first:?}, undone {undone}"),
    );
    keys(view, ":%s#x#z#g<Enter>x");
    check(
        "after :%s the next x deletes the character under Vim's cursor",
        text(view) == " z\ny z\n",
        text(view),
    );
    set_text(view, "x x\ny x\n");
    keys(view, ":s/q/z/<Enter>");
    check(
        ":s without a match leaves the text and says E486",
        text(view) == "x x\ny x\n" && note(view) == splat_i18n::tf("E486: Muster nicht gefunden: {}", &["q"]),
        format!("{:?}, {:?}", text(view), note(view)),
    );
    search(view);
}

fn search(view: &EditorTextView) {
    set_text(view, "one\ntwo\n");
    keys(view, "/");
    let opened = label(view);
    keys(view, "two<Enter>");
    let caret = view.selectedRange().location;
    check(
        "/ opens the search rather than a macro, and two with Enter goes to the match",
        opened == "/" && caret == 4 && label(view) == splat_i18n::t("NORMAL"),
        format!("{opened:?}, caret {caret}, {:?}", label(view)),
    );
    set_text(view, "alpha\nbeta\ngamma beta\n");
    keys(view, "/beta<Enter>");
    let first = (view.selectedRange().location, editor::vim_marked(view));
    keys(view, "n");
    let next = (view.selectedRange().location, editor::vim_marked(view));
    keys(view, ":noh<Enter>");
    let gone = editor::vim_marked(view);
    keys(view, "N:nohlsearch<Enter>");
    let gone_again = editor::vim_marked(view);
    check(
        "the match under the cursor is marked after / and n, and :noh and :nohlsearch take the mark away",
        first == (6, Some(6..10)) && next == (17, Some(17..21)) && gone.is_none() && gone_again.is_none(),
        format!("{first:?} {next:?} {gone:?} {gone_again:?}"),
    );
    set_text(view, "one two\n");
    keys(view, "fn");
    let found = view.selectedRange().location;
    keys(view, "0rn");
    let replaced = text(view);
    keys(view, "dn");
    let refused = (text(view), note(view));
    keys(view, "x");
    check(
        "a key waiting for a character takes n, and dn is not supported and leaves the text",
        found == 1
            && replaced == "nne two\n"
            && refused
                == (
                    "nne two\n".into(),
                    splat_i18n::tf("Befehl nicht unterstützt: {}", &["dn"]),
                )
            && text(view) == "ne two\n",
        format!("{found} {replaced:?} {refused:?} {:?}", text(view)),
    );
    block(view);
}

fn block(view: &EditorTextView) {
    set_text(view, "abc\ndef\nghi\n");
    keys(view, "<C-v>jIX<Esc>");
    let inserted = (text(view), view.selectedRange().location, editor::vim_carets(view));
    let mode = editor::vim_mode(view);
    picture("vim-block-insert");
    keys(view, "u");
    let undone = text(view);
    keys(view, "<C-v>jAX<Esc>");
    let appended = (text(view), view.selectedRange().location);
    check(
        "I and A in Visual block mode type in every line; one Esc leaves one cursor and one u takes it all back",
        inserted == ("Xabc\nXdef\nghi\n".into(), 0, 1)
            && mode == Some(ide_vim::Mode::Normal)
            && undone == "abc\ndef\nghi\n"
            && appended == ("aXbc\ndXef\nghi\n".into(), 1),
        format!("{inserted:?} {mode:?} {undone:?} {appended:?}"),
    );
    let (charwise, _) = one_undo_step(view, "abc def\n", "vl<Esc>gvd");
    let (moved, _) = one_undo_step(view, "abc def\n", "vl<Esc>$gvd");
    set_text(view, "abc\ndef\n");
    keys(view, "Vj<Esc>gv");
    let lines = label(view);
    keys(view, "<Esc><C-v>jl<Esc>gv");
    let columns = label(view);
    keys(view, "<Esc>");
    check(
        "gv selects the last selection again, lines and blocks in their mode",
        charwise == "c def\n"
            && moved == "c def\n"
            && lines == splat_i18n::t("VISUELL ZEILE")
            && columns == splat_i18n::t("VISUELL BLOCK"),
        format!("{charwise:?} {moved:?} {lines:?} {columns:?}"),
    );
    indent(view);
}

fn indent(view: &EditorTextView) {
    let (line, line_undone) = one_undo_step(view, "  a\nb\n", "j==");
    let (whole, whole_undone) = one_undo_step(view, "a {\nb\n    }\n", "gg=G");
    let (visual, visual_undone) = one_undo_step(view, "x {\na\nb\n}\n", "jVj=");
    let (motion, motion_undone) = one_undo_step(view, "x {\na\nb\n}\n", "j=j");
    check(
        "== and = indent like the line above and by the brackets, each one undo step",
        line == "  a\n  b\n"
            && whole == "a {\n    b\n}\n"
            && visual == "x {\n    a\n    b\n}\n"
            && motion == visual
            && line_undone
            && whole_undone
            && visual_undone
            && motion_undone,
        format!("{line:?} {whole:?} {visual:?} {motion:?}"),
    );
    set_text(view, "one two\nthree\n");
    keys(view, "vlldu");
    keys(view, "yyp");
    check(
        "after a visual delete and u, yy and p paste the yanked line",
        text(view) == "one two\none two\nthree\n",
        text(view),
    );
    close_beside();
}

/// `:q` in the editor beside closes only its document; a third one keeps the window open while the file closes.
fn close_beside() {
    if let Some(controller) = window::for_project(&root()) {
        controller.open_file(&root().join(KEEP));
    }
    wait_until(
        || editor_of(KEEP).is_some(),
        |_| {
            let Some(other) = editor_of(OTHER) else {
                return give_up(SCENARIO, "the second file is open beside the first");
            };
            keys(&other, ":q<Enter>");
            wait_until(
                || editor_of(OTHER).is_none(),
                |closed| {
                    check(
                        ":q closes the document of the editor it was typed in, and the one beside stays",
                        closed && editor_of(FILE).is_some(),
                        format!("closed {closed}"),
                    );
                    write_and_close();
                },
            );
        },
    );
}

/// `:q` on the changed file asks as ⌘W does; Abbrechen keeps it.
fn write_and_close() {
    let Some(view) = editor_of(FILE) else {
        return give_up(SCENARIO, "the file is still open");
    };
    set_text(&view, "written\n");
    let question = splat_i18n::tf("Änderungen an „{}“ sichern?", &[FILE]);
    let asked = helpers::answer_alert_saying(question, objc2_app_kit::NSAlertThirdButtonReturn);
    keys(&view, ":q<Enter>");
    wait_until(
        move || asked.get(),
        |answered| {
            check(
                ":q on a changed file asks as ⌘W does, and Abbrechen keeps it open",
                answered && editor_of(FILE).is_some(),
                format!("asked {answered}"),
            );
            write_after_asking();
        },
    );
}

/// `:wq` writes the changed file and closes its tab without asking.
fn write_after_asking() {
    let Some(view) = editor_of(FILE) else {
        return give_up(SCENARIO, "the file is still open");
    };
    keys(&view, "x:wq<Enter>");
    wait_until(
        || editor_of(FILE).is_none(),
        |closed| {
            let disk = std::fs::read_to_string(root().join(FILE)).unwrap_or_default();
            check(
                ":wq writes the changed file and closes it without asking",
                closed && disk == "ritten\n",
                format!("closed {closed}, {disk:?}"),
            );
            reopen(close_unchanged);
        },
    );
}

/// Opens the file again and goes on with `then` once its editor is there.
fn reopen(then: fn(Retained<EditorTextView>)) {
    if let Some(controller) = window::for_project(&root()) {
        controller.open_file(&root().join(FILE));
    }
    wait_until(
        || editor_of(FILE).is_some(),
        move |_| match editor_of(FILE) {
            Some(view) => then(view),
            None => give_up(SCENARIO, "the file opens again"),
        },
    );
}

/// `:x` closes an unchanged file without writing it.
fn close_unchanged(view: Retained<EditorTextView>) {
    let modified = || std::fs::metadata(root().join(FILE)).and_then(|m| m.modified()).ok();
    let before = modified();
    keys(&view, ":x<Enter>");
    wait_until(
        || editor_of(FILE).is_none(),
        move |closed| {
            check(
                ":x closes an unchanged file without writing it",
                closed && before.is_some() && modified() == before,
                format!("closed {closed}"),
            );
            reopen(discard);
        },
    );
}

/// `:q!` closes a changed file without asking, leaves the file as it was saved and keeps no copy of the change.
fn discard(view: Retained<EditorTextView>) {
    keys(&view, "x");
    let copy = editor::recovery_of(&view);
    if let Some(controller) = view.controller() {
        editor::store_copies(&controller);
    }
    keys(&view, ":q!<Enter>");
    let kept = move || {
        ide_document::recovery::Recovery::in_data_dir()
            .pending()
            .unwrap_or_default()
            .iter()
            .any(|pending| pending.id == copy)
    };
    let gone = kept.clone();
    wait_until(
        move || editor_of(FILE).is_none() && !gone(),
        move |_| {
            let disk = std::fs::read_to_string(root().join(FILE)).unwrap_or_default();
            let (closed, kept) = (editor_of(FILE).is_none(), kept());
            check(
                ":q! closes a changed file without asking, the file keeps what was saved and no copy is left",
                closed && disk == "ritten\n" && !kept,
                format!("closed {closed}, {disk:?}, copy kept {kept}"),
            );
            reopen(turn_off);
        },
    );
}

fn turn_off(view: Retained<EditorTextView>) {
    editor::toggle_vim();
    let off = editor::vim_mode(&view);
    check(
        "Editor ▸ Vim-Modus turns it off again",
        off.is_none(),
        format!("{off:?}"),
    );
    let _ = editor::save_all();
    finish(SCENARIO);
}
