//! The `editor-extras` scenario, in a folder the user does not trust: change marks against the last commit read
//! without git, their popover and its revert as one undo step, the marks following a new commit; whitespace shown
//! from the View menu; indentation from `.editorconfig` or the lines, changed from the status bar; and what a save
//! tidies, the line break of a new file and the warning about another charset.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_git::CommitOptions;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{NSAlertSecondButtonReturn, NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::helpers::{self, wait_until};
use super::{check, finish, snapshot};
use crate::editor::{self, ChangeMark as Mark, EditorTextView};
use crate::util::main_soon;
use crate::window::{self, WindowController};

const SCENARIO: &str = "editor-extras";
/// Long enough for the window to draw what changed before a picture, which has no state of its own to wait for.
const DRAWN: Duration = Duration::from_millis(400);
const PROJECT: &str = "extras-project";

const LIB: &str = "src/lib.rs";
const LIB_TEXT: &str = "pub fn one() -> u32 {\n    1\n}\n\npub fn two() -> u32 {\n    2\n}\n\n\
                        pub fn three() -> u32 {\n    3\n}\n";
const EDITED: &str = "pub fn one() -> u32 {\n    10\n}\n\npub fn two() -> u32 {\n    2\n    // note\t  \n}\n\n\
                      pub fn three() -> u32 {\n}\n";
const CONFIG: &str = "root = true\n\n[*.py]\nindent_style = space\nindent_size = 3\n\n[notes.txt]\n\
                      trim_trailing_whitespace = true\ninsert_final_newline = true\ncharset = utf-8-bom\n\n\
                      [fresh.txt]\nend_of_line = crlf\n";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn project_window() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn view(file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*project_window()?, &root().join(file))
}

fn text_of(file: &str) -> String {
    view(file).map(|v| v.document().text().to_string()).unwrap_or_default()
}

fn picture(name: &str) {
    if let Some(window) = project_window().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

fn marks(file: &str) -> Option<editor::ChangesSeen> {
    view(file).map(|v| editor::changes_seen(&v))
}

fn describe(seen: Option<&editor::ChangesSeen>) -> String {
    seen.map_or_else(
        || "no editor".into(),
        |s| format!("head {}, {:?}, popover {:?}", s.head, s.hunks, s.popover),
    )
}

/// A repository with one commit of every file, made with git as a fixture; the app itself reads it without git.
fn make_repository(folder: &Path) -> Result<(), String> {
    let git = super::git::fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    let config = repo.git_dir().join("config");
    let mut content = std::fs::read_to_string(&config).map_err(|e| e.to_string())?;
    content.push_str("[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n");
    std::fs::write(&config, content).map_err(|e| e.to_string())?;
    for (path, text) in [
        (LIB, LIB_TEXT),
        (".editorconfig", CONFIG),
        (
            "tabs.go",
            "package main\n\nfunc main() {\n\tif true {\n\t\tprintln()\n\t}\n}\n",
        ),
        ("two.yaml", "a:\n  b:\n    c: 1\n  d: 2\n"),
        ("script.py", "def main():\n\tif True:\n\t\tpass\n"),
        ("notes.txt", "line one\n"),
        ("fresh.txt", ""),
    ] {
        std::fs::write(folder.join(path), text).map_err(|e| e.to_string())?;
    }
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn commit_again(folder: &Path) -> Result<(), String> {
    let git = super::git::fixture_git()?;
    let repo = git.discover(folder).map_err(|e| e.to_string())?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("Second state", CommitOptions::default())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn run() {
    let folder = helpers::folder(PROJECT);
    std::thread::spawn(move || {
        let made = make_repository(&folder);
        main_soon(move || match made {
            Ok(()) => open(folder),
            Err(error) => {
                check("a test repository is made", false, error);
                finish(SCENARIO);
            }
        });
    });
}

fn open(folder: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, folder);
    let Some(controller) = project_window() else {
        check(
            "the folder opens in a window",
            false,
            format!("{:?}", window::open_projects()),
        );
        return finish(SCENARIO);
    };
    let untrusted = !crate::project::is_trusted(&root());
    if let Some(window) = controller.window()
        && let Some(sheet) = window.attachedSheet()
    {
        window.endSheet_returnCode(&sheet, NSAlertSecondButtonReturn);
    }
    controller.open_file(&root().join(LIB));
    wait_until(
        || marks(LIB).is_some_and(|m| m.head),
        move |ok| unchanged(ok, untrusted),
    );
}

fn unchanged(ok: bool, untrusted: bool) {
    let seen = marks(LIB);
    check(
        "in a folder opened without trust the last commit is read and an unchanged file has no marks",
        ok && untrusted && seen.as_ref().is_some_and(|s| s.hunks.is_empty()),
        format!("untrusted {untrusted}, {}", describe(seen.as_ref())),
    );
    let Some(view) = view(LIB) else { return finish(SCENARIO) };
    let at = |needle: &str| LIB_TEXT.find(needle).unwrap_or(0);
    let one = at("    1\n");
    let two = at("    2\n") + "    2\n".len();
    let three = at("    3\n");
    crate::lsp::apply_text_edits(
        &view,
        vec![
            (one..one + 5, "    10".into()),
            (two..two, "    // note\t  \n".into()),
            (three..three + 6, String::new()),
        ],
    );
    wait_until(
        || {
            marks(LIB).is_some_and(|m| {
                m.hunks
                    == [
                        (Mark::Modified, 1..2),
                        (Mark::Added, 6..7),
                        (Mark::RemovedAbove, 10..10),
                    ]
            })
        },
        marked,
    );
}

fn marked(ok: bool) {
    check(
        "a changed, an added and a removed line are marked after typing pauses",
        ok && text_of(LIB) == EDITED,
        describe(marks(LIB).as_ref()),
    );
    picture("extras-marks");
    let clicked = view(LIB).is_some_and(|v| editor::click_change(&v, 1));
    wait_until(
        || marks(LIB).is_some_and(|m| m.popover.is_some()),
        move |_| popover(clicked),
    );
}

fn popover(clicked: bool) {
    let seen = marks(LIB);
    check(
        "a click on a mark shows the committed lines",
        clicked && seen.as_ref().is_some_and(|s| s.popover.as_deref() == Some("    1\n")),
        describe(seen.as_ref()),
    );
    let Some(view) = view(LIB) else { return finish(SCENARIO) };
    let parts = editor::change_popover(&view);
    if let Some((window, _)) = &parts {
        snapshot(window, "extras-popover");
    }
    let button = parts.map(|(_, button)| button);
    check(
        "the popover offers to revert",
        button
            .as_ref()
            .is_some_and(|b| b.title().to_string() == t("Zurücksetzen")),
        format!("{:?}", button.as_ref().map(|b| b.title().to_string())),
    );
    if let Some(button) = button {
        // SAFETY: A click without a sender, which sends the button's action to its target as the mouse would.
        unsafe { button.performClick(None) };
    }
    let reverted = EDITED.replacen("    10\n", "    1\n", 1);
    wait_until(
        move || text_of(LIB) == reverted && marks(LIB).is_some_and(|m| m.hunks.len() == 2 && m.popover.is_none()),
        reverted_hunk,
    );
}

fn reverted_hunk(ok: bool) {
    check(
        "revert puts the committed lines back and their mark goes",
        ok,
        format!("{:?} / {}", text_of(LIB), describe(marks(LIB).as_ref())),
    );
    let Some(view) = view(LIB) else { return finish(SCENARIO) };
    let undone = view.undoManager().map(|undo| {
        undo.undo();
        let undone = view.document().text().to_string();
        undo.redo();
        undone
    });
    check(
        "one undo takes the revert back",
        undone.as_deref() == Some(EDITED),
        format!("{undone:?}"),
    );
    whitespace(&view);
}

fn find_item(menu: &NSMenu, action: objc2::runtime::Sel) -> Option<Retained<NSMenuItem>> {
    menu.itemArray().iter().find_map(|item| {
        if item.action() == Some(action) {
            return Some(item.clone());
        }
        item.submenu().and_then(|sub| find_item(&sub, action))
    })
}

/// Whitespace shown through the View menu's action, with its check mark; ⌥⌘I belongs to the inspector (E94, E101).
fn whitespace(view: &EditorTextView) {
    let mtm = view.mtm();
    let item = NSApplication::sharedApplication(mtm)
        .mainMenu()
        .and_then(|m| find_item(&m, objc2::sel!(ideToggleWhitespace:)));
    let shortcut = item
        .as_ref()
        .map(|i| (i.title().to_string(), i.keyEquivalent().to_string()));
    check(
        "the View menu shows whitespace, without the inspector's ⌥⌘I",
        shortcut
            .as_ref()
            .is_some_and(|(title, key)| title == t("Leerzeichen anzeigen") && key.is_empty()),
        format!("{shortcut:?}"),
    );
    let was = editor::whitespace_visible(view);
    // SAFETY: The action the menu item sends, which takes the sender, here none.
    let _: () = unsafe { msg_send![view, ideToggleWhitespace: Option::<&AnyObject>::None] };
    let checked = item.as_ref().is_some_and(|item| {
        // SAFETY: The method AppKit calls to validate a menu item, with that item.
        let _: bool = unsafe { msg_send![view, validateMenuItem: &**item] };
        item.state() == objc2_app_kit::NSControlStateValueOn
    });
    check(
        "the menu item shows whitespace in the editor and gets its check mark",
        !was && editor::whitespace_visible(view) && checked,
        format!(
            "before {was}, after {}, checked {checked}",
            editor::whitespace_visible(view)
        ),
    );
    helpers::after(DRAWN, || {
        picture("extras-whitespace");
        save_and_commit();
    });
}

fn save_and_commit() {
    let Some(view) = view(LIB) else { return finish(SCENARIO) };
    editor::save_view(&view);
    let folder = root();
    std::thread::spawn(move || {
        let committed = commit_again(&folder);
        main_soon(move || match committed {
            Ok(()) => wait_until(
                || marks(LIB).is_some_and(|m| m.head && m.hunks.is_empty()),
                committed_marks,
            ),
            Err(error) => {
                check("the change is committed", false, error);
                finish(SCENARIO);
            }
        });
    });
}

fn committed_marks(ok: bool) {
    check(
        "a new commit seen by the watcher becomes the marks' base",
        ok,
        describe(marks(LIB).as_ref()),
    );
    let Some(controller) = project_window() else {
        return finish(SCENARIO);
    };
    for file in ["tabs.go", "two.yaml", "script.py"] {
        controller.open_file(&root().join(file));
    }
    wait_until(
        || {
            ["tabs.go", "two.yaml", "script.py"]
                .iter()
                .all(|file| view(file).is_some())
        },
        |_| indentation(),
    );
}

fn shown(file: &str) -> (String, Option<String>) {
    view(file).map(|v| editor::indent_shown(&v)).unwrap_or_default()
}

/// Indentation guessed from the lines or taken from `.editorconfig`, and changed through the status bar's menu.
fn indentation() {
    let tabs = shown("tabs.go");
    check(
        "a file indented with tabs says so in the status bar",
        tabs.0 == tf("Tabs: {}", &["4"]),
        format!("{tabs:?}"),
    );
    let two = shown("two.yaml");
    check(
        "a file indented by two spaces says so",
        two.0 == tf("Leerzeichen: {}", &["2"]),
        format!("{two:?}"),
    );
    let python = shown("script.py");
    check(
        ".editorconfig wins over the lines",
        python.0 == tf("Leerzeichen: {}", &["3"]),
        format!("{python:?}"),
    );
    let Some(view) = view("script.py") else {
        return finish(SCENARIO);
    };
    let end = view.document().text().len_utf16();
    view.setSelectedRange(NSRange::new(end, 0));
    // SAFETY: The action Tab sends, which takes the sender, here none.
    let _: () = unsafe { msg_send![&*view, insertTab: Option::<&AnyObject>::None] };
    let typed = view.document().text().to_string();
    check(
        "Tab inserts the configured spaces",
        typed.ends_with("pass\n   "),
        format!("{typed:?}"),
    );
    let interval = |v: &EditorTextView| v.defaultParagraphStyle().map_or(0.0, |s| s.defaultTabInterval());
    let narrow = interval(&view);
    choose(&view, 1);
    choose(&view, 108);
    let wide = interval(&view);
    let after = shown("script.py");
    check(
        "the status bar's menu switches the file to tabs eight columns wide",
        after.0 == tf("Tabs: {}", &["8"]) && view.ivars().indent.borrow().as_str() == "\t" && wide > narrow * 2.4,
        format!("{after:?}, tab {narrow:.1} → {wide:.1}"),
    );
    helpers::after(DRAWN, || {
        picture("extras-indentation");
        tidy();
    });
}

/// Sends what an entry of the indentation menu sends.
fn choose(view: &EditorTextView, tag: isize) {
    let item = crate::util::tagged(view.mtm(), "", objc2::sel!(ideSetIndent:), "", tag);
    // SAFETY: The action the menu's entries send, with such an entry as the sender.
    let _: () = unsafe { msg_send![view, ideSetIndent: &*item] };
}

/// A save trims line ends and adds the last break; a file with no break yet gets the configured one.
fn tidy() {
    let Some(controller) = project_window() else {
        return finish(SCENARIO);
    };
    controller.open_file(&root().join("notes.txt"));
    let Some(notes) = view("notes.txt") else {
        return finish(SCENARIO);
    };
    let warning = editor::indent_shown(&notes).1;
    check(
        "a charset other than .editorconfig asks for is pointed out",
        warning.as_deref() == Some(tf("Zeichensatz laut .editorconfig: {}", &["utf-8-bom"]).as_str()),
        format!("{warning:?}"),
    );
    let end = notes.document().text().len_utf16();
    notes.setSelectedRange(NSRange::new(end, 0));
    editor::type_text(&notes, "second line   ");
    picture("extras-charset");
    // Saved in the same turn of the run loop as the typing, as a busy main thread runs ⌘S.
    editor::save_view(&notes);
    let disk = std::fs::read_to_string(root().join("notes.txt")).unwrap_or_default();
    check(
        "saving trims whitespace at line ends and adds the last line break",
        disk == "line one\nsecond line\n",
        format!("{disk:?}"),
    );
    let undone = notes.undoManager().map(|undo| {
        undo.undo();
        let text = notes.document().text().to_string();
        undo.redo();
        text
    });
    check(
        "one undo takes the tidying back",
        undone.as_deref() == Some("line one\nsecond line   "),
        format!("{undone:?}"),
    );
    if let Some(controller) = project_window() {
        controller.open_file(&root().join("fresh.txt"));
    }
    let Some(fresh) = view("fresh.txt") else {
        return finish(SCENARIO);
    };
    editor::type_text(&fresh, "a");
    editor::type_newline(&fresh);
    editor::type_text(&fresh, "b");
    let text = fresh.document().text().to_string();
    check(
        "a new line in a file without breaks gets the configured break",
        text == "a\r\nb",
        format!("{text:?}"),
    );
    done();
}

fn done() {
    if let Some(view) = view(LIB) {
        // SAFETY: The action the menu item sends, which takes the sender, here none.
        let _: () = unsafe { msg_send![&*view, ideToggleWhitespace: Option::<&AnyObject>::None] };
    }
    let saved = editor::save_all();
    check(
        "every editor is saved before quitting",
        saved,
        format!("{} unsaved", editor::dirty_count()),
    );
    finish(SCENARIO);
}
