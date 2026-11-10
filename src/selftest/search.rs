//! The `search` scenario: the files panel's header creates a file and a folder in the selected folder and folds every
//! folder; ⌘⇧F focuses the field of the search panel; a query lists its matches grouped per file, follows the case,
//! whole-word and regex options and the file filters, says when it stops at its limit or cannot read a pattern, opens
//! a match with its text selected, follows unsaved text in that editor and a file written on disk; replacing works on
//! one line of a closed file, on an open file as one undo step and, after a question, on everything, keeping a byte
//! order mark and CRLF line endings; Esc empties the field.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu};
use splat_i18n::{t, tf};

use super::{check, finish, snapshot};
use crate::util::main_after;
use crate::window::{self, Panel, SearchToggle as Toggle, WindowController, search_selftest as search};

const SCENARIO: &str = "search";
const WAIT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(200);
const MANY: usize = 6_000;
const PICTURE_DELAY: Duration = Duration::from_millis(400);

const MAIN: &str = "fn main() {\n    let needle = 1;\n    println!(\"Needle {needle}\");\n}\n";

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("search-project");
    let many: String = (0..MANY).map(|n| format!("hit {n}\n")).collect();
    for (path, text) in [
        ("src/main.rs", MAIN),
        ("src/lib.rs", "pub fn needles() {}\n// NEEDLE in capitals\n"),
        ("docs/notes.md", "a needle in the docs\n"),
        ("many.txt", many.as_str()),
        (".gitignore", "target/\n"),
        ("target/ignored.txt", "needle\n"),
    ] {
        let file = folder.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&folder));
        let _ = std::fs::write(file, text);
    }
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    if window_of(&root).is_none() {
        check(
            "the folder opens in a window",
            false,
            format!("{:?}", window::open_projects()),
        );
        finish(SCENARIO);
        return;
    }
    check_menu(mtm);
    super::files_header::run(root, searching);
}

/// After the files header: ⌘⇧F, then the first query.
fn searching(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    controller.activate_panel(Panel::Agents);
    controller.show_search();
    let seen = controller.search_panel().map(|p| search::seen(&p));
    let inspected = controller.inspect();
    check(
        "⌘⇧F shows the search panel with the focus in its field",
        inspected.panel == Some(Panel::Search)
            && inspected.panel_title == Panel::Search.title()
            && seen.as_ref().is_some_and(|s| s.field_focused && !s.results_shown),
        format!("{:?}, focused {:?}", inspected.panel, seen.map(|s| s.field_focused)),
    );
    with_panel(&root, |p| search::type_query(p, "needle"));
    poll(root, Instant::now() + WAIT, settled, found);
}

/// The menu item behind ⌘⇧F: Bearbeiten ▸ Suchen ▸ Im Projekt suchen …
fn check_menu(mtm: MainThreadMarker) {
    let menu = NSApplication::sharedApplication(mtm).mainMenu();
    let item = menu.and_then(|m| find_item(&m, objc2::sel!(ideSearchProject:)));
    let shortcut = item.as_ref().map(|i| {
        let mask = i.keyEquivalentModifierMask();
        (
            i.keyEquivalent().to_string(),
            mask.contains(NSEventModifierFlags::Command) && mask.contains(NSEventModifierFlags::Shift),
            i.title().to_string(),
        )
    });
    check(
        "the menu bar offers the project search on ⌘⇧F",
        shortcut
            .as_ref()
            .is_some_and(|(key, mask, title)| key == "f" && *mask && title == t("Im Projekt suchen …")),
        format!("{shortcut:?}"),
    );
}

fn find_item(menu: &NSMenu, action: objc2::runtime::Sel) -> Option<Retained<objc2_app_kit::NSMenuItem>> {
    menu.itemArray().iter().find_map(|item| {
        if item.action() == Some(action) {
            return Some(item.clone());
        }
        item.submenu().and_then(|sub| find_item(&sub, action))
    })
}

fn window_of(root: &Path) -> Option<Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root))
}

fn with_panel(root: &Path, f: impl FnOnce(&window::SearchPanel)) {
    if let Some(panel) = window_of(root).and_then(|c| c.search_panel()) {
        f(&panel);
    }
}

/// Calls `then` once `ready` holds for the search panel of `root`, or with false when time is up.
fn poll(root: PathBuf, until: Instant, ready: fn(&search::Seen) -> bool, then: fn(PathBuf, bool)) {
    let done = window_of(&root)
        .and_then(|c| c.search_panel())
        .is_some_and(|p| ready(&search::seen(&p)));
    if done || Instant::now() >= until {
        then(root, done);
        return;
    }
    main_after(POLL, move || poll(root, until, ready, then));
}

fn settled(seen: &search::Seen) -> bool {
    seen.settled
}

fn seen(root: &Path) -> Option<search::Seen> {
    window_of(root).and_then(|c| c.search_panel()).map(|p| search::seen(&p))
}

fn files(seen: &search::Seen) -> String {
    seen.files
        .iter()
        .map(|(path, n)| format!("{path}: {n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn expect_files(seen: &search::Seen, expected: &[(&str, usize)]) -> bool {
    seen.files.len() == expected.len()
        && seen
            .files
            .iter()
            .zip(expected)
            .all(|((path, n), (want, m))| path == want && n == m)
}

fn found(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "a query lists its matches grouped per file, without ignored files",
        ok && expect_files(&seen, &[("docs/notes.md", 1), ("src/lib.rs", 2), ("src/main.rs", 3)]),
        files(&seen),
    );
    check(
        "every file and matching line has a row",
        seen.rows == 8 && seen.results_shown,
        format!("{} rows, shown {}", seen.rows, seen.results_shown),
    );
    let counted = tf("{} Treffer in {} Dateien", &["6", "3"]);
    check(
        "the status counts matches and files",
        seen.status == counted,
        seen.status.clone(),
    );
    let labels = window_of(&root)
        .and_then(|c| c.search_panel())
        .map(|p| (search::row_label(&p, 0), search::row_label(&p, 1)));
    check(
        "VoiceOver reads a file with its count and a match with its line",
        labels.as_ref().is_some_and(|(file, line)| {
            file.as_deref() == Some(tf("{}, 1 Treffer, in {}", &["notes.md", "docs"]).as_str())
                && line.as_deref() == Some(tf("Zeile {}: {}", &["1", "a needle in the docs"]).as_str())
        }),
        format!("{labels:?}"),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-results");
    }
    with_panel(&root, |p| search::set(p, Toggle::Case, true));
    poll(root, Instant::now() + WAIT, settled, case_sensitive);
}

fn case_sensitive(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "matching case leaves out NEEDLE and Needle",
        ok && expect_files(&seen, &[("docs/notes.md", 1), ("src/lib.rs", 1), ("src/main.rs", 2)]),
        files(&seen),
    );
    with_panel(&root, |p| {
        search::set(p, Toggle::Case, false);
        search::set(p, Toggle::Word, true);
    });
    poll(root, Instant::now() + WAIT, settled, whole_word);
}

fn whole_word(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "a whole word leaves out needles",
        ok && expect_files(&seen, &[("docs/notes.md", 1), ("src/lib.rs", 1), ("src/main.rs", 3)]),
        files(&seen),
    );
    with_panel(&root, |p| {
        search::set(p, Toggle::Word, false);
        search::set(p, Toggle::Case, true);
        search::set(p, Toggle::Regex, true);
        search::type_query(p, r"fn \w+\(");
    });
    poll(root, Instant::now() + WAIT, settled, regex);
}

fn regex(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "a regular expression finds both function heads",
        ok && expect_files(&seen, &[("src/lib.rs", 1), ("src/main.rs", 1)]),
        files(&seen),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-regex");
    }
    with_panel(&root, |p| search::type_query(p, "fn ("));
    poll(root, Instant::now() + WAIT, settled, invalid);
}

fn invalid(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    let prefix = tf("Ungültiges Suchmuster: {}", &[""]);
    check(
        "a pattern that is no regular expression says so instead of matching",
        ok && seen.files.is_empty() && seen.status.starts_with(&prefix),
        seen.status.clone(),
    );
    with_panel(&root, |p| {
        search::set(p, Toggle::Regex, false);
        search::set(p, Toggle::Case, false);
        search::set_globs(p, "src", "lib.rs");
        search::type_query(p, "needle");
    });
    poll(root, Instant::now() + WAIT, settled, filtered);
}

fn filtered(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "the file filters include a folder and exclude a file",
        ok && expect_files(&seen, &[("src/main.rs", 3)]),
        files(&seen),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-filters");
    }
    with_panel(&root, |p| {
        search::set_globs(p, "", "");
        search::type_query(p, "hit");
    });
    poll(root, Instant::now() + WAIT, settled, limited);
}

fn limited(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    let expected = tf(
        "Mehr als {} Treffer — Suche eingrenzen",
        &[&window::SEARCH_LIMIT.to_string()],
    );
    check(
        "a search stops at its limit and asks to narrow it",
        ok && seen.status == expected && seen.files.first().is_some_and(|(_, n)| *n == window::SEARCH_LIMIT),
        format!("{} ({})", seen.status, files(&seen)),
    );
    with_panel(&root, |p| search::type_query(p, "needle"));
    poll(root, Instant::now() + WAIT, settled, open_match);
}

fn open_match(root: PathBuf, ok: bool) {
    let opened = ok
        && window_of(&root)
            .and_then(|c| c.search_panel())
            .is_some_and(|p| search::open(&p, "src/main.rs", 2));
    // The window shows what changed only after this turn of the run loop, and a picture should show it.
    main_after(PICTURE_DELAY, move || opened_match(root, opened));
}

fn opened_match(root: PathBuf, opened: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let file = root.join("src/main.rs");
    let editor = crate::editor::editor_of(&controller, &file);
    let selected = editor.as_ref().map(|e| e.selectedRange());
    let focused = editor.as_ref().is_some_and(|e| {
        controller
            .window()
            .and_then(|w| w.firstResponder())
            .is_some_and(|r| Retained::as_ptr(&r).cast::<()>() == Retained::as_ptr(e).cast())
    });
    let at = MAIN.find("needle").map(|byte| MAIN[..byte].encode_utf16().count());
    check(
        "Return on a match opens its file with the match selected and focused",
        opened && selected.is_some_and(|r| Some(r.location) == at && r.length == 6) && focused,
        format!("{selected:?}, expected at {at:?}, focused {focused}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "search-editor");
    }
    if let Some(editor) = editor {
        let end = editor.document().text().len_utf16();
        editor.setSelectedRange(objc2_foundation::NSRange::new(end, 0));
        crate::editor::type_text(&editor, "// needle, not saved yet\n");
    }
    poll(root, Instant::now() + WAIT, buffer_counted, unsaved);
}

fn buffer_counted(seen: &search::Seen) -> bool {
    seen.settled && seen.files.iter().any(|(path, n)| path == "src/main.rs" && *n == 4)
}

fn unsaved(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "the matches follow an editor's unsaved text, searched in place of its file",
        ok,
        files(&seen),
    );
    let _ = std::fs::write(root.join("src/fresh.rs"), "// a needle written by another program\n");
    poll(root, Instant::now() + WAIT, disk_counted, written);
}

fn disk_counted(seen: &search::Seen) -> bool {
    seen.settled && seen.files.iter().any(|(path, _)| path == "src/fresh.rs")
}

fn written(root: PathBuf, ok: bool) {
    let Some(seen) = seen(&root) else {
        return finish(SCENARIO);
    };
    check(
        "a file written on disk while matches are shown joins them",
        ok && expect_files(
            &seen,
            &[
                ("docs/notes.md", 1),
                ("src/fresh.rs", 1),
                ("src/lib.rs", 2),
                ("src/main.rs", 4),
            ],
        ),
        files(&seen),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-refreshed");
    }
    crate::editor::save_all();
    super::search_replace::run(root, escape);
}

fn escape(root: PathBuf) {
    with_panel(&root, search::escape);
    main_after(PICTURE_DELAY, move || escaped(root));
}

fn escaped(root: PathBuf) {
    let back = seen(&root);
    check(
        "Esc empties the field and clears the matches",
        back.as_ref()
            .is_some_and(|s| !s.results_shown && s.field_focused && s.files.is_empty()),
        format!(
            "shown {:?}, focused {:?}",
            back.as_ref().map(|s| s.results_shown),
            back.as_ref().map(|s| s.field_focused)
        ),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-empty");
    }
    finish(SCENARIO);
}
