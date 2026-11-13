//! The `search` scenario: the files panel's header creates a file and a folder in the selected folder and folds every
//! folder; ⌘⇧F focuses the field of the search panel; a query lists its matches grouped per file, follows the case,
//! whole-word and regex options and the file filters, says when it stops at its limit or cannot read a pattern, opens
//! a match with its text selected, follows unsaved text in that editor and a file written on disk; replacing works on
//! one line of a closed file, on an open file as one undo step and, after a question, on everything, keeping a byte
//! order mark and CRLF line endings; Esc empties the field.

use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu};
use splat_i18n::{t, tf};

use super::helpers::wait_until;
use super::{check, finish, helpers, snapshot};
use crate::window::{self, Panel, SearchToggle as Toggle, WindowController, search_selftest as search};

const SCENARIO: &str = "search";
const MANY: usize = 6_000;

const MAIN: &str = "fn main() {\n    let needle = 1;\n    println!(\"Needle {needle}\");\n}\n";

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let many: String = (0..MANY).map(|n| format!("hit {n}\n")).collect();
    let files = [
        ("src/main.rs", MAIN),
        ("src/lib.rs", "pub fn needles() {}\n// NEEDLE in capitals\n"),
        ("docs/notes.md", "a needle in the docs\n"),
        ("many.txt", many.as_str()),
        (".gitignore", "target/\n"),
        ("target/ignored.txt", "needle\n"),
    ];
    helpers::open_project(SCENARIO, "search-project", &files, move |_| {
        check_menu(mtm);
        super::files_header::run(helpers::root("search-project"), searching);
    });
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
    poll(root, settled, found);
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
fn poll(root: PathBuf, ready: fn(&search::Seen) -> bool, then: fn(PathBuf, bool)) {
    let probe = root.clone();
    wait_until(
        move || seen(&probe).is_some_and(|s| ready(&s)),
        move |ok| then(root, ok),
    );
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
    chevrons_line_up(&root);
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-results");
    }
    with_panel(&root, |p| search::set(p, Toggle::Case, true));
    poll(root, settled, case_sensitive);
}

/// A closed folder's chevron in the folder tree begins where the search panel's closed chevron does.
fn chevrons_line_up(root: &Path) {
    let controller = window_of(root);
    let search = controller
        .as_ref()
        .and_then(|c| c.search_panel())
        .and_then(|p| search::replace_chevron(&p))
        .and_then(|button| ink_start(&button));
    let tree = controller
        .as_ref()
        .and_then(|c| c.file_tree_outline())
        .and_then(|outline| closed_folder_chevron(&outline));
    let pitches = (
        controller
            .as_ref()
            .and_then(|c| c.file_tree_outline())
            .filter(|outline| outline.numberOfRows() > 1)
            .map(|outline| outline.rectOfRow(1).origin.y - outline.rectOfRow(0).origin.y),
        controller
            .as_ref()
            .and_then(|c| c.search_panel())
            .and_then(|p| search::row_pitch(&p)),
    );
    check(
        "the folder tree's rows are as tall as the search panel's",
        matches!(pitches, (Some(tree), Some(search)) if (tree - search).abs() < 0.5),
        format!("{pitches:?}"),
    );
    check(
        "a closed folder's chevron in the files panel begins where the search panel's does",
        search
            .zip(tree)
            .is_some_and(|(search, tree)| (search - tree).abs() < 0.5),
        format!("search {search:?}, folder tree {tree:?}"),
    );
}

fn closed_folder_chevron(outline: &objc2_app_kit::NSOutlineView) -> Option<f64> {
    (0..outline.numberOfRows()).find_map(|row| {
        let item = outline.itemAtRow(row)?;
        // SAFETY: The item belongs to the outline, which asks its own data source.
        let closed = unsafe { outline.isExpandable(Some(&item)) && !outline.isItemExpanded(Some(&item)) };
        if !closed {
            return None;
        }
        let cell = outline
            .viewAtColumn_row_makeIfNecessary(0, row, true)?
            .downcast::<objc2_app_kit::NSTableCellView>()
            .ok()?;
        // SAFETY: The cell is one of the folder tree's, which set an image view on it.
        let image = unsafe { cell.imageView() }?;
        ink_start(&image)
    })
}

/// Where the first drawn column of `view` lies, in window coordinates.
fn ink_start(view: &objc2_app_kit::NSView) -> Option<f64> {
    let bounds = view.bounds();
    let bitmap = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    let (wide, high) = (bitmap.pixelsWide(), bitmap.pixelsHigh());
    let column = (0..wide).find(|&x| {
        (0..high).any(|y| {
            bitmap
                .colorAtX_y(x, y)
                .is_some_and(|color| color.alphaComponent() > 0.3)
        })
    })?;
    let scale = wide as f64 / bounds.size.width.max(1.0);
    let start = objc2_foundation::NSRect::new(
        objc2_foundation::NSPoint::new(bounds.origin.x + column as f64 / scale, bounds.origin.y),
        objc2_foundation::NSSize::new(1.0, 1.0),
    );
    Some(view.convertRect_toView(start, None).origin.x)
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
    poll(root, settled, whole_word);
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
    poll(root, settled, regex);
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
    poll(root, settled, invalid);
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
    poll(root, settled, filtered);
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
    poll(root, settled, limited);
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
    poll(root, settled, open_match);
}

fn open_match(root: PathBuf, ok: bool) {
    let opened = ok
        && window_of(&root)
            .and_then(|c| c.search_panel())
            .is_some_and(|p| search::open(&p, "src/main.rs", 2));
    let probe = root.clone();
    wait_until(move || match_opened(&probe).0, move |_| opened_match(root, opened));
}

/// Whether the editor of main.rs has the first needle selected and the focus, with its selection.
fn match_opened(root: &Path) -> (bool, Option<objc2_foundation::NSRange>, bool) {
    let Some(controller) = window_of(root) else {
        return (false, None, false);
    };
    let editor = crate::editor::editor_of(&controller, &root.join("src/main.rs"));
    let selected = editor.as_ref().map(|e| e.selectedRange());
    let focused = editor.as_ref().is_some_and(|e| {
        controller
            .window()
            .and_then(|w| w.firstResponder())
            .is_some_and(|r| Retained::as_ptr(&r).cast::<()>() == Retained::as_ptr(e).cast())
    });
    let at = MAIN.find("needle").map(|byte| MAIN[..byte].encode_utf16().count());
    let ok = selected.is_some_and(|r| Some(r.location) == at && r.length == 6) && focused;
    (ok, selected, focused)
}

fn opened_match(root: PathBuf, opened: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let editor = crate::editor::editor_of(&controller, &root.join("src/main.rs"));
    let (ok, selected, focused) = match_opened(&root);
    check(
        "Return on a match opens its file with the match selected and focused",
        opened && ok,
        format!("{selected:?}, focused {focused}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "search-editor");
    }
    if let Some(editor) = editor {
        let end = editor.document().text().len_utf16();
        editor.setSelectedRange(objc2_foundation::NSRange::new(end, 0));
        crate::editor::type_text(&editor, "// needle, not saved yet\n");
    }
    poll(root, buffer_counted, unsaved);
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
    poll(root, disk_counted, written);
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
    let probe = root.clone();
    wait_until(
        move || seen(&probe).is_some_and(|s| !s.results_shown && s.files.is_empty()),
        move |_| escaped(root),
    );
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
