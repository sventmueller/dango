//! The `debug-menus` scenario (B1, E22, E94, E101): what the debugger does through its menus, sheets, rows and keys.
//! The configuration menu offers `.vscode/launch.json`'s entries, greys out one no debugger reads, runs one on the
//! scripted adapter, attaches one to a process picked in a sheet and says when an adapter is missing; the Debug menu
//! detaches, steps, restarts, switches and removes all breakpoints; the gutter's context menu sets, switches,
//! conditions and removes breakpoints through its sheets; the panel's checkboxes, Return, Delete, double click and
//! context menus act on their rows; the rotor steps through the marked lines; the inspector's field and the console's
//! take what is typed; a program the adapter wants in a terminal runs in a tab; an untrusted folder starts nothing
//! until the panel's trust button is answered.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSEventModifierFlags, NSMenu, NSModalResponse,
    NSPopUpButton, NSTextField, NSView,
};
use splat_i18n::t;

use super::a11y::keys::press;
use super::debug::{project, script};
use super::helpers::{editor, wait_until, window_of};
use super::{check, finish, give_up};
use crate::debug::probe::{self as debug, Command};
use crate::window::{Panel, WindowController};

const SCENARIO: &str = "debug-menus";
/// How long a command that should start nothing is given to show that it did not.
const NOTHING_STARTS: Duration = Duration::from_millis(500);

const LAUNCH_JSON: &str = r#"{
    "version": "0.2.0",
    "configurations": [
        {"type": "fake", "request": "launch", "name": "Scripted run", "program": "${workspaceFolder}/src/main.rs"},
        {"type": "fake", "request": "attach", "name": "Scripted attach", "pid": "${command:pickProcess}"},
        {"type": "cppvsdbg", "request": "launch", "name": "Windows only"},
        {"type": "node", "request": "launch", "name": "Node app", "program": "${workspaceFolder}/app.js"}
    ]
}"#;

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = project("debug-menus");
    let _ = super::helpers::make_project("debug-menus", &[(".vscode/launch.json", LAUNCH_JSON)]);
    crate::app::open_project(mtm, root.clone());
    let probe = root.clone();
    wait_until(
        move || front(&probe).is_some_and(|c| !debug::configuration_items(&c).is_empty()),
        move |_| configurations(root),
    );
}

fn front(root: &Path) -> Option<Retained<WindowController>> {
    window_of(root)
}

fn seen(root: &Path) -> debug::Seen {
    front(root).and_then(|c| debug::seen(&c)).unwrap_or_default()
}

/// The console titled `title`: its text and whether it takes input.
fn console(root: &Path, title: &str) -> Option<(String, bool)> {
    seen(root)
        .consoles
        .into_iter()
        .rev()
        .find(|(name, _, _)| name == title)
        .map(|(_, text, input)| (text, input))
}

/// The file the scripted adapter of a launch.json entry runs through.
fn default_script() -> PathBuf {
    ide_dap::fake::Script::default().path
}

fn has_sheet(root: &Path) -> bool {
    front(root)
        .and_then(|c| c.window())
        .is_some_and(|w| w.attachedSheet().is_some())
}

/// Every view inside `view`.
fn views_in(view: &NSView, out: &mut Vec<Retained<NSView>>) {
    for child in view.subviews().iter() {
        views_in(&child, out);
        out.push(child);
    }
}

/// The texts of the sheet in front of the window, its pop-up menu's entries, and puts `typed` into its text field.
fn read_sheet(root: &Path, typed: Option<&str>) -> (Vec<String>, Vec<String>) {
    let mut views = Vec::new();
    if let Some(content) = front(root)
        .and_then(|c| c.window())
        .and_then(|w| w.attachedSheet())
        .and_then(|s| s.contentView())
    {
        views_in(&content, &mut views);
    }
    let mut texts = Vec::new();
    let mut entries = Vec::new();
    for view in &views {
        if let Some(popup) = view.downcast_ref::<NSPopUpButton>() {
            entries.extend(popup.itemTitles().iter().map(|t| t.to_string()));
        } else if let Some(field) = view.downcast_ref::<NSTextField>() {
            if field.isEditable() {
                if let Some(typed) = typed {
                    field.setStringValue(&crate::util::ns(typed));
                }
            } else {
                texts.push(field.stringValue().to_string());
            }
        }
    }
    (texts, entries)
}

/// Ends the sheet in front of the window as a click on its button `response` does.
fn answer_sheet(root: &Path, response: NSModalResponse) {
    if let Some(window) = front(root).and_then(|c| c.window())
        && let Some(sheet) = window.attachedSheet()
    {
        window.endSheet_returnCode(&sheet, response);
    }
}

/// The Debug menu of the menu bar, its items validated as before it opens.
fn debug_menu() -> Option<Retained<NSMenu>> {
    let mtm = MainThreadMarker::new()?;
    let main = NSApplication::sharedApplication(mtm).mainMenu()?;
    let menu = main.itemWithTitle(&crate::util::ns(t("Debuggen")))?.submenu()?;
    menu.update();
    Some(menu)
}

/// The Debug menu's items: title and whether it is enabled.
fn menu_entries() -> Vec<(String, bool)> {
    debug_menu().map_or_else(Vec::new, |menu| {
        menu.itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| (item.title().to_string(), item.isEnabled()))
            .collect()
    })
}

/// Chooses the enabled item `title` of the Debug menu; whether there was one.
fn choose_in_menu(title: &str) -> bool {
    let Some(menu) = debug_menu() else { return false };
    let index = menu.indexOfItemWithTitle(&crate::util::ns(title));
    let enabled = menu.itemAtIndex(index).is_some_and(|item| item.isEnabled());
    if enabled {
        menu.performActionForItemAtIndex(index);
    }
    enabled
}

/// The launch.json entries come in the configuration menu, the one without a debugger greyed out, and one of them
/// runs on the scripted adapter to its end.
fn configurations(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder opens");
    };
    controller.activate_panel(Panel::Debug);
    let items = debug::configuration_items(&controller);
    let expected = [
        (t("Aktuelles Programm debuggen"), true),
        ("Scripted run", true),
        ("Scripted attach", true),
        ("Windows only", false),
        ("Node app", true),
        (t("An Prozess anhängen …"), true),
    ];
    check(
        "the configuration menu offers launch.json's entries and greys out the one no debugger reads",
        items.iter().map(|(t, on)| (t.as_str(), *on)).eq(expected),
        format!("{items:?}"),
    );
    let started = debug::start_configuration(&controller, "Scripted run");
    let chosen = debug::chosen_configuration(&controller);
    let probe = root.clone();
    wait_until(
        move || console(&probe, "Scripted run").is_some_and(|(text, input)| !input && text.contains(t("Beendet"))),
        move |ok| {
            let text = console(&root, "Scripted run").map(|(text, _)| text).unwrap_or_default();
            let ended = t("Das Programm endete mit Code {}.").replace("{}", "0");
            check(
                "a chosen configuration is kept and runs on its adapter to its end, which its console tells",
                started
                    && chosen.as_deref() == Some("Scripted run")
                    && ok
                    && text.contains("fake program started")
                    && text.contains(&ended),
                format!("started {started}, chosen {chosen:?}, console {text:?}"),
            );
            attach_picked(root);
        },
    );
}

/// A configuration that asks for a process shows the running ones in a sheet and attaches to the one picked; Stop
/// then detaches.
fn attach_picked(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    // The scripted adapter of a launch.json entry runs its default script, which an attached session then walks
    // through; a breakpoint there holds it while it is attached.
    debug::toggle_breakpoint(&controller, &default_script(), 0);
    debug::start_configuration(&controller, "Scripted attach");
    let probe = root.clone();
    wait_until(
        move || has_sheet(&probe),
        move |shown| {
            let (texts, entries) = read_sheet(&root, None);
            check(
                "a configuration that asks for a process lists the running ones in a sheet",
                shown
                    && texts.iter().any(|text| text == t("An Prozess anhängen"))
                    && entries.first().is_some_and(|e| e.ends_with(')')),
                format!("{texts:?}, {} processes, first {:?}", entries.len(), entries.first()),
            );
            answer_sheet(&root, NSAlertFirstButtonReturn);
            let probe = root.clone();
            wait_until(
                move || {
                    seen(&probe)
                        .sessions
                        .iter()
                        .any(|(name, phase)| name == "Scripted attach" && phase.starts_with(t("Angehalten")))
                },
                move |ok| {
                    check(
                        "the picked process is attached to and stops at the breakpoint",
                        ok,
                        format!("{:?}", seen(&root).sessions),
                    );
                    detach(root);
                },
            );
        },
    );
}

fn detach(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let titled = menu_entries().iter().any(|(title, on)| title == t("Trennen") && *on);
    let chosen = choose_in_menu(t("Trennen"));
    check(
        "the Debug menu's Stop reads Detach for an attached program",
        titled && chosen,
        format!("{:?}", menu_entries()),
    );
    debug::toggle_breakpoint(&controller, &default_script(), 0);
    let probe = root.clone();
    wait_until(
        move || console(&probe, "Scripted attach").is_some_and(|(_, input)| !input),
        move |ok| {
            let phases = seen(&root).sessions;
            check(
                "Stop lets the attached process go and ends the session",
                ok && phases
                    .iter()
                    .any(|(name, phase)| name == "Scripted attach" && phase.starts_with(t("Beendet"))),
                format!("{phases:?}"),
            );
            attach_cancelled(root);
        },
    );
}

/// "An Prozess anhängen …" asks for a process too; cancelling the sheet starts nothing.
fn attach_cancelled(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let before = seen(&root).sessions.len();
    let _ = controller;
    let asked = choose_in_menu(t("An Prozess anhängen …"));
    let probe = root.clone();
    wait_until(
        move || has_sheet(&probe),
        move |shown| {
            answer_sheet(&root, NSAlertSecondButtonReturn);
            let probe = root.clone();
            wait_until(
                move || !has_sheet(&probe),
                move |_| {
                    let Some(controller) = front(&root) else {
                        return give_up(SCENARIO, "the folder's window is open");
                    };
                    let after = seen(&root).sessions.len();
                    check(
                        "the Debug menu's attaching asks for a process, and cancelling starts nothing",
                        asked && shown && !has_sheet(&root) && !debug::is_picking(&controller) && after == before,
                        format!("sheet {shown}, {before} sessions before, {after} after"),
                    );
                    missing_adapter(root);
                },
            );
        },
    );
}

/// A configuration whose adapter is missing says so in its console; js-debug is offered for installation in a sheet
/// when Node is there, and the sheet is cancelled, as nothing may be downloaded here.
fn missing_adapter(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    debug::start_configuration(&controller, "Node app");
    let probe = root.clone();
    wait_until(
        move || console(&probe, "Node app").is_some_and(|(_, input)| !input),
        move |ended| {
            let probe = root.clone();
            let offer_settled = move || {
                let text = console(&probe, "Node app").map(|(text, _)| text).unwrap_or_default();
                let needs_node = text.contains(&splat_i18n::tf("{} fehlt; es braucht {}.", &["js-debug", "node"]));
                has_sheet(&probe) != needs_node
            };
            wait_until(offer_settled, move |_| {
                let text = console(&root, "Node app").map(|(text, _)| text).unwrap_or_default();
                let needs_node = text.contains(&splat_i18n::tf("{} fehlt; es braucht {}.", &["js-debug", "node"]));
                let (texts, _) = read_sheet(&root, None);
                let offered = texts.iter().any(|text| text == t("vscode-js-debug installieren?"));
                check(
                    "a missing adapter is named in the console, and js-debug is offered when Node is installed",
                    ended && text.contains("js-debug") && (offered != needs_node),
                    format!("console {text:?}, sheet {texts:?}"),
                );
                answer_sheet(&root, NSAlertSecondButtonReturn);
                let probe = root.clone();
                wait_until(move || !has_sheet(&probe), move |_| current_program(root));
            });
        },
    );
}

/// "Aktuelles Programm debuggen" from the Debug menu names a file no debugger reads without a configuration, and
/// without a file in front starts nothing.
fn current_program(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let notes = root.join("notes.md");
    let _ = std::fs::write(&notes, "# Notes\n");
    controller.open_file(&notes);
    let chosen = choose_in_menu(t("Aktuelles Programm debuggen"));
    let probe = root.clone();
    wait_until(
        move || console(&probe, "notes.md").is_some_and(|(_, input)| !input),
        move |ok| {
            let text = console(&root, "notes.md").map(|(text, _)| text).unwrap_or_default();
            let words = t("Für {}-Dateien gibt es keinen Debugger ohne Startkonfiguration.");
            let (start, end) = words.split_once("{}").unwrap_or((words, ""));
            check(
                "debugging a Markdown file as the current program says no debugger reads it without a configuration",
                chosen && ok && text.contains(start) && text.contains(end),
                format!("{text:?}"),
            );
            library_file(root);
        },
    );
}

/// A file of a library without a program or tests of its own starts no session, and the console says why.
fn library_file(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let package = root.join("shapes");
    let _ = std::fs::create_dir_all(package.join("src"));
    let _ = std::fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"shapes\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    );
    let _ = std::fs::write(package.join("src/lib.rs"), "pub mod shape;\n");
    let shape = package.join("src/shape.rs");
    let _ = std::fs::write(&shape, "use std::fmt;\n\npub struct Shape {\n    pub sides: u8,\n}\n");
    controller.open_file(&shape);
    let chosen = choose_in_menu(t("Aktuelles Programm debuggen"));
    let probe = root.clone();
    wait_until(
        move || console(&probe, "shape.rs").is_some_and(|(_, input)| !input),
        move |ok| {
            let Some(controller) = front(&root) else {
                return give_up(SCENARIO, "the folder's window is open");
            };
            let text = console(&root, "shape.rs").map(|(text, _)| text).unwrap_or_default();
            let words = t(
                "„{}“ ist eine Bibliothek ohne Programm, und diese Datei hat keine Tests. Debugge eine Datei mit main oder Tests, oder lege in .vscode/launch.json fest, welches Programm startet.",
            );
            let (start, end) = words.split_once("{}").unwrap_or((words, ""));
            let sessions = seen(&root).sessions;
            check(
                "debugging a library's file without tests starts nothing, says the library has no program and not \
                 that macOS is asked",
                chosen
                    && ok
                    && text.contains(start)
                    && text.contains(end)
                    && text.contains("shapes")
                    && !text.contains(t("Wartet auf Freigabe durch macOS …")),
                format!("{text:?}, sessions {sessions:?}"),
            );
            controller.open_welcome();
            let before = seen(&root).sessions;
            let chosen = choose_in_menu(t("Aktuelles Programm debuggen"));
            super::helpers::after(NOTHING_STARTS, move || {
                let after = seen(&root).sessions;
                check(
                    "without a file in front, debugging the current program starts nothing",
                    chosen && after == before,
                    format!("{before:?}, then {after:?}"),
                );
                gutter_menu(root);
            });
        },
    );
}

/// The non-separator entries of the gutter's menu at `line`: title and whether it is enabled.
fn gutter_entries(view: &crate::editor::EditorTextView, line: usize) -> Vec<(String, bool)> {
    crate::debug::gutter_menu(view, line).map_or_else(Vec::new, |menu| {
        menu.itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| (item.title().to_string(), item.isEnabled()))
            .collect()
    })
}

/// Chooses the entry `title` of the gutter's menu at `line`, as a click on it does.
fn choose(view: &crate::editor::EditorTextView, line: usize, title: &str) -> bool {
    let Some(menu) = crate::debug::gutter_menu(view, line) else {
        return false;
    };
    let index = menu.indexOfItemWithTitle(&crate::util::ns(title));
    if index < 0 {
        return false;
    }
    menu.performActionForItemAtIndex(index);
    true
}

/// Asks the breakpoint sheet of `title` at `line`, types `text` and ends it with `response`.
fn through_sheet(root: &Path, line: usize, title: &str, text: &str, response: NSModalResponse) -> bool {
    let Some(view) = front(root).and_then(|c| editor(&c, "main.rs")) else {
        return false;
    };
    let chosen = choose(&view, line, title);
    let shown = has_sheet(root);
    read_sheet(root, Some(text));
    answer_sheet(root, response);
    chosen && shown
}

/// The gutter's context menu sets a breakpoint, switches it off and on, gives it a condition and a hit count through
/// its sheets, makes a log point on another line, and removes the breakpoint.
fn gutter_menu(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let file = root.join("src/main.rs");
    controller.open_file(&file);
    let Some(view) = editor(&controller, "main.rs") else {
        return give_up(SCENARIO, "the program opens in an editor");
    };
    let empty = gutter_entries(&view, 1);
    check(
        "the gutter's menu on a line without a breakpoint offers to set one, with a condition or as a log point",
        empty
            == [
                (t("Haltepunkt setzen").to_string(), true),
                (t("Bedingung …").to_string(), true),
                (t("Logmeldung …").to_string(), true),
            ],
        format!("{empty:?}"),
    );
    let set = choose(&view, 1, t("Haltepunkt setzen"));
    let full = gutter_entries(&view, 1);
    let switched_off = choose(&view, 1, t("Haltepunkt ausschalten"));
    let off = debug::breakpoint_at(&controller, &file, 1);
    let switch_title = gutter_entries(&view, 1).first().map(|(t, _)| t.clone());
    let switched_on = choose(&view, 1, t("Haltepunkt einschalten"));
    let on = debug::breakpoint_at(&controller, &file, 1);
    check(
        "the menu sets a breakpoint, then offers to switch it off and on, its condition, hit count, log message and \
         removal",
        set && switched_off
            && switched_on
            && full.iter().map(|(t, _)| t.as_str()).eq([
                t("Haltepunkt ausschalten"),
                t("Bedingung …"),
                t("Trefferzahl …"),
                t("Logmeldung …"),
                t("Haltepunkt entfernen"),
            ])
            && off.as_ref().is_some_and(|b| !b.3)
            && switch_title.as_deref() == Some(t("Haltepunkt einschalten"))
            && on.as_ref().is_some_and(|b| b.3),
        format!("{full:?}, off {off:?}, then {switch_title:?}, on {on:?}"),
    );
    let condition = through_sheet(&root, 1, t("Bedingung …"), "hits > 2", NSAlertFirstButtonReturn);
    let hits = through_sheet(&root, 1, t("Trefferzahl …"), ">= 3", NSAlertFirstButtonReturn);
    let cancelled = through_sheet(&root, 1, t("Logmeldung …"), "ignored", NSAlertSecondButtonReturn);
    let log = through_sheet(&root, 2, t("Logmeldung …"), "at {hits}", NSAlertFirstButtonReturn);
    let probe = root.clone();
    wait_until(
        move || !has_sheet(&probe),
        move |_| {
            sheets_answered(root, [condition, hits, cancelled, log]);
        },
    );
}

fn sheets_answered(root: PathBuf, asked: [bool; 4]) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let file = root.join("src/main.rs");
    let first = debug::breakpoint_at(&controller, &file, 1);
    let second = debug::breakpoint_at(&controller, &file, 2);
    let look = editor(&controller, "main.rs").map(|view| debug::gutter(&view, 2).0);
    check(
        "the sheets give the breakpoint its condition and hit count, a cancelled one changes nothing, and a log \
         message on a line without one makes a log point",
        asked.iter().all(|a| *a)
            && first
                .as_ref()
                .is_some_and(|b| b.0.as_deref() == Some("hits > 2") && b.1.as_deref() == Some(">= 3") && b.2.is_none())
            && second.as_ref().is_some_and(|b| b.2.as_deref() == Some("at {hits}"))
            && look.flatten().is_some_and(|l| !debug::is_outlined(Some(l))),
        format!("asked {asked:?}, line 2: {first:?}, line 3: {second:?}, {look:?}"),
    );
    let removed = editor(&controller, "main.rs").is_some_and(|view| choose(&view, 1, t("Haltepunkt entfernen")));
    let gone = debug::breakpoint_at(&controller, &file, 1).is_none();
    check(
        "the menu removes the breakpoint",
        removed && gone,
        format!("chosen {removed}, gone {gone}"),
    );
    rows(root);
}

/// A breakpoint's checkbox in the panel switches it, Return on its row shows its line and on the section folds it.
fn rows(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let file = root.join("src/main.rs");
    debug::toggle_breakpoint(&controller, &file, 3);
    let row = debug::breakpoint_row(&controller, &file, 3);
    let clicked = row.is_some_and(|r| debug::click_switch(&controller, r));
    let off = debug::breakpoint_at(&controller, &file, 3).is_some_and(|b| !b.3);
    let row = debug::breakpoint_row(&controller, &file, 3);
    let again = row.is_some_and(|r| debug::click_switch(&controller, r));
    let on = debug::breakpoint_at(&controller, &file, 3).is_some_and(|b| b.3);
    check(
        "a breakpoint's checkbox in the panel switches it off and on",
        clicked && off && again && on,
        format!("row {row:?}, off {off}, on {on}"),
    );
    controller.open_welcome();
    let focused = debug::breakpoint_row(&controller, &file, 3).is_some_and(|row| debug::focus_list(&controller, row));
    let went = controller
        .window()
        .map(|w| press(&w, "\r", 36, NSEventModifierFlags::empty()).to_string());
    let shown = controller
        .selected_tab()
        .and_then(|tab| crate::editor::editor_of_tab(&controller, tab))
        .map(|view| {
            let text = super::helpers::text_of(&view);
            let caret = view.selectedRange().location;
            let line = text.chars().take(caret).filter(|c| *c == '\n').count();
            (view.document().path().map(Path::to_path_buf), line)
        });
    check(
        "Return on a breakpoint's row shows its line in the editor",
        focused
            && shown
                .as_ref()
                .is_some_and(|(path, line)| path.as_ref() == Some(&file) && *line == 3),
        format!("{went:?}, {shown:?}"),
    );
    let section = debug::breakpoints_section_row(&controller);
    let open = section.is_some_and(|r| debug::is_open(&controller, r));
    if let Some(r) = section {
        debug::activate(&controller, r);
    }
    let folded = section.is_some_and(|r| !debug::is_open(&controller, r));
    if let Some(r) = section {
        debug::activate(&controller, r);
    }
    let reopened = debug::breakpoints_section_row(&controller).is_some_and(|r| debug::is_open(&controller, r));
    check(
        "Return on the breakpoints' heading folds and unfolds the section",
        open && folded && reopened,
        format!("open {open}, folded {folded}, open again {reopened}"),
    );
    row_menus(root);
}

/// A breakpoint row's context menu offers what the gutter's does, Delete removes the breakpoint, and the Debug menu
/// switches all breakpoints off and on again.
fn row_menus(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let file = root.join("src/main.rs");
    let menu = debug::breakpoint_row(&controller, &file, 3).and_then(|r| debug::context_menu(&controller, false, r));
    let titles: Vec<String> = menu.map_or_else(Vec::new, |m| {
        m.itemArray()
            .iter()
            .filter(|i| !i.isSeparatorItem())
            .map(|i| i.title().to_string())
            .collect()
    });
    check(
        "a breakpoint row's context menu offers to switch, condition and remove it",
        titles.first().map(String::as_str) == Some(t("Haltepunkt ausschalten"))
            && titles.last().map(String::as_str) == Some(t("Haltepunkt entfernen")),
        format!("{titles:?}"),
    );
    let log_row = debug::breakpoint_row(&controller, &file, 2);
    let focused = log_row.is_some_and(|r| debug::focus_list(&controller, r));
    let went = controller
        .window()
        .map(|w| press(&w, "\u{7f}", 51, NSEventModifierFlags::empty()).to_string());
    check(
        "Delete on a breakpoint's row removes it",
        focused && debug::breakpoint_at(&controller, &file, 2).is_none(),
        format!("row {log_row:?}, {went:?}"),
    );
    let all_off = choose_in_menu(t("Haltepunkte ausschalten"));
    let look_off = editor(&controller, "main.rs").and_then(|view| debug::gutter(&view, 3).0);
    let renamed = menu_entries()
        .iter()
        .any(|(title, _)| title == t("Haltepunkte einschalten"));
    let all_on = choose_in_menu(t("Haltepunkte einschalten"));
    let look_on = editor(&controller, "main.rs").and_then(|view| debug::gutter(&view, 3).0);
    check(
        "the Debug menu switches all breakpoints off, draws them outlined and offers to switch them on again",
        all_off && debug::is_outlined(look_off) && renamed && all_on && !debug::is_outlined(look_on),
        format!("{look_off:?}, then {look_on:?}, {:?}", menu_entries()),
    );
    debug::toggle_breakpoint(&controller, &file, 6);
    let scripted = ide_dap::fake::Script {
        restart: true,
        panics: true,
        ..script(&root)
    };
    debug::start_fake(&controller, scripted);
    let probe = root.clone();
    wait_until(
        move || seen(&probe).paused.is_some_and(|(_, line)| line == 3),
        move |ok| {
            check(
                "the session stops at the first breakpoint",
                ok,
                format!("{:?}", seen(&root).paused),
            );
            paused(root);
        },
    );
}

/// Paused: an exception filter's checkbox, selecting, double-clicking and activating frames, and the rotor.
fn paused(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let before = debug::filters(&controller);
    let clicked = debug::filter_row(&controller, "all").is_some_and(|r| debug::click_switch(&controller, r));
    let after = debug::filters(&controller);
    check(
        "an exception filter's checkbox in the panel switches it",
        clicked && before.iter().any(|(id, on)| id == "all" && !on) && after.iter().any(|(id, on)| id == "all" && *on),
        format!("{before:?} -> {after:?}"),
    );
    let outer = debug::frame_row(&controller, 1);
    if let Some(row) = outer {
        debug::select_row(&controller, row);
    }
    let selected_outer = debug::selected_frame(&controller);
    if let Some(row) = debug::frame_row(&controller, 0) {
        debug::double_click(&controller, row);
    }
    let double_clicked = debug::selected_frame(&controller);
    if let Some(row) = debug::frame_row(&controller, 1) {
        debug::activate(&controller, row);
    }
    let returned = debug::selected_frame(&controller);
    check(
        "selecting a frame's row, a double click and Return on others choose the frame shown",
        outer.is_some()
            && selected_outer.is_some_and(|(_, frame)| frame == 1)
            && double_clicked.is_some_and(|(_, frame)| frame == 0)
            && returned.is_some_and(|(_, frame)| frame == 1),
        format!("row {outer:?}: {selected_outer:?}, {double_clicked:?}, {returned:?}"),
    );
    if let Some(view) = editor(&controller, "main.rs") {
        let first = debug::rotor_step(&view, None, true);
        let second = first
            .as_ref()
            .and_then(|(_, at)| debug::rotor_step(&view, Some(*at), true));
        let back = second
            .as_ref()
            .and_then(|(_, at)| debug::rotor_step(&view, Some(*at), false));
        let beyond = second
            .as_ref()
            .and_then(|(_, at)| debug::rotor_step(&view, Some(*at), true));
        check(
            "the rotor \"Haltepunkte\" steps to the paused line and the next breakpoint and back",
            first
                .as_ref()
                .is_some_and(|(label, _)| label.contains(t("Programm hier angehalten")))
                && second.as_ref().is_some_and(|(label, _)| label.ends_with('7'))
                && back == first
                && beyond.is_none(),
            format!("{first:?}, {second:?}, back {back:?}, beyond {beyond:?}"),
        );
    }
    debug::type_watch(&controller, "hits");
    debug::type_watch(&controller, "line");
    debug::type_in_console(&controller, "line");
    debug::type_in_console(&controller, "nothing_by_this_name");
    let probe = root.clone();
    wait_until(
        move || {
            let seen = seen(&probe);
            seen.watches.len() == 2
                && seen.watches.iter().all(|(_, v)| v.is_some())
                && seen
                    .consoles
                    .iter()
                    .any(|(_, text, _)| text.contains("› line\n") && text.contains("› nothing_by_this_name\n"))
        },
        move |ok| typed(root, ok),
    );
}

/// The inspector's field added watches, the console evaluated; ↑ and ↓ go through what was asked, and a watch goes
/// through its row's context menu or Delete.
fn typed(root: PathBuf, ok: bool) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let watches = seen(&root).watches;
    let text = console(&root, "main.rs").map(|(text, _)| text).unwrap_or_default();
    let back = debug::console_arrow(&controller, true);
    let further = debug::console_arrow(&controller, true);
    let forward = debug::console_arrow(&controller, false);
    let reset = debug::console_arrow(&controller, false);
    check(
        "the inspector's field adds watches, the console evaluates, and ↑ and ↓ go through what was asked",
        ok && back.as_deref() == Some("nothing_by_this_name")
            && further.as_deref() == Some("line")
            && forward.as_deref() == Some("nothing_by_this_name")
            && reset.as_deref() == Some(""),
        format!("{watches:?}, {text:?}, ↑ {back:?} {further:?}, ↓ {forward:?} {reset:?}"),
    );
    let menu = debug::watch_row(&controller, 0).and_then(|r| debug::context_menu(&controller, true, r));
    let index = menu.as_ref().map_or(-1, |m| {
        m.indexOfItemWithTitle(&crate::util::ns(t("Ausdruck entfernen")))
    });
    if let Some(menu) = menu.as_ref().filter(|_| index >= 0) {
        menu.performActionForItemAtIndex(index);
    }
    let left = seen(&root).watches;
    let deleted = debug::watch_row(&controller, 0).is_some_and(|r| debug::delete_row(&controller, true, r));
    check(
        "a watch goes through its row's context menu, and another by Delete",
        index >= 0 && left.len() == 1 && left[0].0 == "line" && deleted && seen(&root).watches.is_empty(),
        format!("menu entry {index}, then {left:?}, then {:?}", seen(&root).watches),
    );
    commands(root);
}

/// The title row's Step Into and Step Out, and the Debug menu's Restart on an adapter that can restart.
fn commands(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let into = debug::click_control(&controller, Command::StepInto);
    let probe = root.clone();
    wait_until(
        move || seen(&probe).paused.is_some_and(|(_, line)| line == 4),
        move |stepped_in| {
            let Some(controller) = front(&root) else {
                return give_up(SCENARIO, "the folder's window is open");
            };
            let out = debug::click_control(&controller, Command::StepOut);
            let probe = root.clone();
            wait_until(
                move || seen(&probe).paused.is_some_and(|(_, line)| line > 4),
                move |stepped_out| {
                    check(
                        "the title row's Step Into and Step Out move on through the program",
                        into && out && stepped_in && stepped_out,
                        format!("{:?}", seen(&root).paused),
                    );
                    let restart = choose_in_menu(t("Neu starten"));
                    let probe = root.clone();
                    wait_until(
                        move || seen(&probe).paused.is_some_and(|(_, line)| line == 3),
                        move |ok| restarted(root, ok && restart),
                    );
                },
            );
        },
    );
}

fn restarted(root: PathBuf, ok: bool) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    check(
        "the Debug menu's Restart starts the program over to the first breakpoint, as the adapter can restart",
        ok,
        format!("{:?}", seen(&root).paused),
    );
    controller.open_welcome();
    let shown = choose_in_menu(t("Debug-Konsole zeigen"));
    let front_tab = controller
        .selected_tab()
        .and_then(|id| controller.tab_ids().iter().position(|t| *t == id))
        .and_then(|index| controller.tab_titles().get(index).cloned());
    check(
        "the Debug menu brings the console to the front",
        shown && front_tab == Some(splat_i18n::tf("Debug-Konsole: {}", &["main.rs"])),
        format!("{front_tab:?}"),
    );
    let listed = crate::debug::activity(&controller);
    crate::debug::show(&controller);
    let panel = controller.inspect().panel;
    if let Some((id, _, _)) = listed.first() {
        crate::debug::stop(&controller, *id);
    }
    check(
        "the activity view lists the session, shows the debug panel and stops the session",
        listed.len() == 1 && panel == Some(Panel::Debug),
        format!("{listed:?}, {panel:?}"),
    );
    let probe = root.clone();
    wait_until(
        move || seen(&probe).controls.iter().all(|on| !on),
        move |_| {
            let cleared = choose_in_menu(t("Alle Haltepunkte entfernen"));
            check(
                "the Debug menu removes every breakpoint",
                cleared && seen(&root).breakpoints.is_empty(),
                format!("{:?}", seen(&root).breakpoints),
            );
            terminal(root);
        },
    );
}

/// An adapter that asks for its program in a terminal gets a terminal tab, which then owns the program.
fn terminal(root: PathBuf) {
    let Some(controller) = front(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let program = root.join("run.sh");
    let _ = std::fs::write(&program, "#!/bin/sh\necho scripted program in a terminal\n");
    let _ = std::fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755));
    let tabs = controller.tab_titles().len();
    let scripted = ide_dap::fake::Script {
        path: program,
        lines: vec![1, 2],
        run_in_terminal: true,
        ..script(&root)
    };
    debug::start_fake(&controller, scripted);
    let probe = root.clone();
    wait_until(
        move || {
            front(&probe).is_some_and(|c| c.tab_titles().len() > tabs)
                && console(&probe, "run.sh").is_some_and(|(_, input)| !input)
        },
        move |ok| {
            let titles = front(&root).map(|c| c.tab_titles()).unwrap_or_default();
            check(
                "a program the adapter wants in a terminal runs in a new tab, and the session ends after it",
                ok,
                format!("{titles:?}, console {:?}", console(&root, "run.sh")),
            );
            untrusted();
        },
    );
}

/// A folder opened without trust: its debugger reads no launch.json and starts nothing; the panel's trust button asks
/// again, and once trusted the configurations come.
fn untrusted() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let files = [
        (".vscode/launch.json", LAUNCH_JSON),
        ("src/main.rs", super::debug::PROGRAM),
    ];
    let folder = super::helpers::make_project("debug-untrusted", &files)
        .unwrap_or_else(|_| super::helpers::root("debug-untrusted"));
    super::helpers::open_beside(mtm, &folder);
    let probe = folder.clone();
    wait_until(
        move || has_sheet(&probe),
        move |asked| {
            answer_sheet(&folder, NSAlertSecondButtonReturn);
            let probe = folder.clone();
            wait_until(move || !has_sheet(&probe), move |_| restricted(folder, asked));
        },
    );
}

fn restricted(folder: PathBuf, asked: bool) {
    let Some(controller) = front(&folder) else {
        return give_up(SCENARIO, "the untrusted folder opens");
    };
    controller.activate_panel(Panel::Debug);
    controller.open_file(&folder.join("src/main.rs"));
    let items = debug::configuration_items(&controller);
    let state = debug::trust_state(&controller);
    let menu = menu_entries();
    debug::start_fake(&controller, script(&folder));
    check(
        "in an untrusted folder the debugger reads no launch.json, greys out starting, says why, and starts nothing",
        asked
            && items.len() == 2
            && state == Some((true, false))
            && menu.iter().any(|(title, on)| title == t("Starten") && !on)
            && seen(&folder).consoles.is_empty()
            && seen(&folder).sessions.is_empty(),
        format!(
            "{items:?}, note and start {state:?}, {menu:?}, {:?}",
            seen(&folder).consoles
        ),
    );
    let pressed = debug::press_trust(&controller);
    let probe = folder.clone();
    wait_until(
        move || has_sheet(&probe),
        move |shown| {
            answer_sheet(&folder, NSAlertFirstButtonReturn);
            let probe = folder.clone();
            let configured = move || {
                front(&probe).is_some_and(|c| {
                    debug::configuration_items(&c)
                        .iter()
                        .any(|(title, _)| title == "Scripted run")
                })
            };
            wait_until(configured, move |_| {
                let controller = front(&folder);
                let items = controller.as_ref().map(debug::configuration_items).unwrap_or_default();
                let state = controller.as_ref().and_then(debug::trust_state);
                check(
                    "the panel's trust button asks again, and once trusted launch.json's configurations come",
                    pressed
                        && shown
                        && items.iter().any(|(title, _)| title == "Scripted run")
                        && state == Some((false, true)),
                    format!("{items:?}, note and start {state:?}"),
                );
                finish(SCENARIO);
            });
        },
    );
}
