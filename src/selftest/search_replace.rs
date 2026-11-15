//! Replacing in the search panel, part of the `search` scenario: one line of a closed file, an open file as one undo
//! step, and after a question every match, which keeps a byte order mark and CRLF line endings.

use std::path::{Path, PathBuf};

use objc2_app_kit::{NSAlertFirstButtonReturn, NSTextField, NSView};
use splat_i18n::{t, tf};

use super::helpers::wait_until;
use super::{check, snapshot};
use crate::window::{self, search_selftest as search};

const CRLF: &[u8] = b"\xEF\xBB\xBFa needle\r\nsecond line\r\n";

fn panel(root: &Path) -> Option<objc2::rc::Retained<window::SearchPanel>> {
    window::for_project(root).and_then(|c| c.search_panel())
}

fn seen(root: &Path) -> Option<search::Seen> {
    panel(root).map(|p| search::seen(&p))
}

fn listed(root: &Path, relative: &str) -> bool {
    seen(root).is_some_and(|s| s.settled && s.files.iter().any(|(path, _)| path == relative))
}

fn main_text(root: &Path) -> String {
    window::for_project(root)
        .and_then(|c| crate::editor::editor_of(&c, &root.join("src/main.rs")))
        .map(|e| e.document().text().to_string())
        .unwrap_or_default()
}

pub(crate) fn run(root: PathBuf, then: fn(PathBuf)) {
    let _ = std::fs::write(root.join("crlf.txt"), CRLF);
    let probe = root.clone();
    wait_until(
        move || listed(&probe, "crlf.txt"),
        move |_| {
            if let Some(panel) = panel(&root) {
                search::replace_with(&panel, "pin");
            }
            let probe = root.clone();
            wait_until(
                move || seen(&probe).is_some_and(|s| s.settled && s.can_replace_all),
                move |_| line(root, then),
            );
        },
    );
}

/// The replace button of a line replaces that line's matches in a file no editor shows, which is written at once.
fn line(root: PathBuf, then: fn(PathBuf)) {
    if let Some(window) = window::for_project(&root).and_then(|c| c.window()) {
        snapshot(&window, "search-replace");
    }
    let hidden = panel(&root).is_some_and(|p| search::click_replace(&p, "docs/notes.md", Some(1)));
    let notes = root.join("docs/notes.md");
    let probe = root.clone();
    wait_until(
        move || {
            std::fs::read_to_string(&notes).is_ok_and(|t| t == "a pin in the docs\n")
                && !listed(&probe, "docs/notes.md")
        },
        move |ok| {
            check(
                "a line's replace button, there only while the pointer is on the row, replaces in a closed file",
                ok && hidden,
                std::fs::read_to_string(root.join("docs/notes.md")).unwrap_or_default(),
            );
            file(root, then);
        },
    );
}

/// The replace button of a file replaces in its open editor, unsaved, as one step that one undo takes back.
fn file(root: PathBuf, then: fn(PathBuf)) {
    let before = main_text(&root);
    let on_disk = std::fs::read_to_string(root.join("src/main.rs")).unwrap_or_default();
    let clicked = panel(&root).is_some_and(|p| search::click_replace(&p, "src/main.rs", None));
    let after = main_text(&root);
    let replaced = !after.to_lowercase().contains("needle") && after.contains("pin");
    let unsaved = std::fs::read_to_string(root.join("src/main.rs")).unwrap_or_default() == on_disk;
    let editor = window::for_project(&root).and_then(|c| crate::editor::editor_of(&c, &root.join("src/main.rs")));
    if let Some(undo) = editor.as_ref().and_then(|e| e.undoManager()) {
        undo.undo();
    }
    let undone = main_text(&root) == before;
    check(
        "a file's replace button replaces in its open editor, unsaved, as one undo step",
        clicked && replaced && unsaved && undone,
        format!("replaced {replaced}, unsaved {unsaved}, undone {undone}: {after:?}"),
    );
    // The matches follow the undo once it rests.
    let probe = root.clone();
    wait_until(
        move || {
            seen(&probe).is_some_and(|s| {
                s.settled && s.can_replace_all && s.files.iter().any(|(path, n)| path == "src/main.rs" && *n == 4)
            })
        },
        move |_| everything(root, then),
    );
}

/// "Alle ersetzen" asks with the count, then replaces everywhere; the matches go and a note says how many went.
fn everything(root: PathBuf, then: fn(PathBuf)) {
    let counts = seen(&root).map(|s| (s.files.iter().map(|(_, n)| n).sum::<usize>(), s.files.len()));
    if let Some(panel) = panel(&root) {
        search::replace_all(&panel);
    }
    let window = window::for_project(&root).and_then(|c| c.window());
    let sheet = window.as_ref().and_then(|w| w.attachedSheet());
    let question = sheet
        .as_ref()
        .and_then(|s| s.contentView())
        .map(|content| texts(&content))
        .unwrap_or_default();
    let asked = counts.is_some_and(|(matches, files)| {
        question.contains(&tf(
            "{} Treffer in {} Dateien ersetzen?",
            &[&matches.to_string(), &files.to_string()],
        ))
    });
    if let (Some(window), Some(sheet)) = (window, sheet) {
        window.endSheet_returnCode(&sheet, NSAlertFirstButtonReturn);
    }
    let probe = root.clone();
    wait_until(
        move || seen(&probe).is_some_and(|s| s.settled && s.files.is_empty()),
        move |ok| {
            let crlf = std::fs::read(root.join("crlf.txt")).unwrap_or_default();
            let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap_or_default();
            let note = window::for_project(&root).and_then(|c| c.notice());
            check(
                "Alle ersetzen asks with the counts, then replaces in every file",
                asked && ok && lib == "pub fn pins() {}\n// pin in capitals\n" && !main_text(&root).contains("needle"),
                format!("{question:?}, {counts:?}, lib {lib:?}"),
            );
            check(
                "a closed file keeps its byte order mark and CRLF line endings",
                crlf == b"\xEF\xBB\xBFa pin\r\nsecond line\r\n",
                format!("{crlf:?}"),
            );
            let expected =
                counts.map(|(m, f)| tf("{} Treffer in {} Dateien ersetzt", &[&m.to_string(), &f.to_string()]));
            check(
                "a note says how many were replaced, and the results are empty",
                note.is_some() && note == expected && seen(&root).is_some_and(|s| s.status == t("Keine Treffer")),
                format!("{note:?}"),
            );
            if let Some(window) = window::for_project(&root).and_then(|c| c.window()) {
                snapshot(&window, "search-replaced");
            }
            crate::editor::save_all();
            then(root);
        },
    );
}

fn texts(view: &NSView) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        found.push(field.stringValue().to_string());
    }
    for child in view.subviews().iter() {
        found.extend(texts(&child));
    }
    found
}
