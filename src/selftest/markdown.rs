//! The `markdown` scenario (E59): a Markdown file opens with its preview beside it, which ⇧⌘V closes and opens and which
//! closes with its editor; the format commands toggle and are one undo step each; HTML on a pasteboard goes in as
//! Markdown; the document exports to PDF, HTML and Word; the Format menu has splat's shortcuts.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypeString};
use objc2_foundation::{NSData, NSRange};
use splat_i18n::{t, tf};

use super::helpers::{editor, text_of, utf16_len, wait_until, window_of};
use super::{check, finish, snapshot};
use crate::editor::EditorTextView;
use crate::markdown::selftest::{
    Command, Kind, Outcome, Source, export_seen, export_to, link_with, paste_from, source_of,
};
use crate::util::{constant, main_after, ns};

const SCENARIO: &str = "markdown";

const NOTE: &str = "# Notiz\n\nEin Wort hier.\n\nZweite Zeile\n\n![bild](bild.png)\n";

/// A picture of 4 × 4 pixels.
const PNG: [u8; 73] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00, 0x04, 0x08, 0x02, 0x00, 0x00, 0x00, 0x26, 0x93, 0x09, 0x29, 0x00, 0x00, 0x00, 0x10, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xb8, 0xe0, 0xa0, 0x00, 0x47, 0x0c, 0xc4, 0x71, 0x00, 0xf9, 0x43, 0x13, 0x01,
    0xd7, 0x3a, 0xcd, 0xee, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("markdown-project");
    let _ = std::fs::create_dir_all(&folder);
    let root = folder.canonicalize().unwrap_or(folder);
    let _ = std::fs::write(root.join("notiz.md"), NOTE);
    let _ = std::fs::write(root.join("zweite.md"), "# Zweite\n");
    let _ = std::fs::write(root.join("dritte.md"), "# Dritte\n");
    let _ = std::fs::write(root.join("bild.png"), PNG);
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(500), move || preview_beside(root));
}

/// The editor of `name` in the window of `root`.
fn editor_of(root: &Path, name: &str) -> Option<objc2::rc::Retained<EditorTextView>> {
    editor(&*window_of(root)?, name)
}

fn preview_beside(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        check("the folder gets its window", false, root.display().to_string());
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("notiz.md"));
    let preview = tf("Vorschau: {}", &["notiz.md"]);
    let has_preview = |c: &crate::window::WindowController| c.window_tabs().iter().any(|docs| docs.contains(&preview));
    check(
        "a Markdown file opens with its preview beside it in its tab",
        controller
            .window_tabs()
            .iter()
            .any(|docs| docs.len() == 2 && docs[0] == "notiz.md" && docs[1] == preview),
        format!("{:?}", controller.window_tabs()),
    );
    let Some(view) = editor(&controller, "notiz.md") else {
        return finish(SCENARIO);
    };
    crate::preview::toggle_beside(&view);
    let closed = !has_preview(&controller);
    crate::preview::toggle_beside(&view);
    check(
        "⇧⌘V closes the preview and opens it again",
        closed && has_preview(&controller),
        format!("{:?}", controller.window_tabs()),
    );
    controller.preview_file(&root.join("zweite.md"));
    controller.preview_file(&root.join("dritte.md"));
    let previews = |c: &crate::window::WindowController| {
        c.tab_titles()
            .into_iter()
            .filter(|title| title.ends_with("zweite.md") || title.ends_with("dritte.md"))
            .filter(|title| title != "zweite.md" && title != "dritte.md")
            .collect::<Vec<_>>()
    };
    let opened = previews(&controller);
    if let Some(tab) = super::helpers::tab(&controller, "dritte.md") {
        controller.close_tab(tab);
    }
    let closed = previews(&controller);
    check(
        "each clicked file opens in a tab of its own, and the preview that opened with it goes with its editor",
        opened.len() == 2 && closed == [tf("Vorschau: {}", &["zweite.md"])],
        format!("{opened:?} / {closed:?}"),
    );
    formats(&root, &view);
    pasting(&root, &view);
    menus(&root);
    super::markdown_editor::run(&root);
    super::split::run(&root);
    exports(root);
}

/// Runs `command` with `range` selected; the text afterwards.
fn after(view: &EditorTextView, range: NSRange, command: Command) -> String {
    view.setSelectedRange(range);
    crate::markdown::perform(view, command as isize);
    text_of(view)
}

fn at(text: &str, part: &str) -> NSRange {
    let start = text.find(part).map_or(0, |byte| utf16_len(&text[..byte]));
    NSRange::new(start, utf16_len(part))
}

fn formats(_root: &Path, view: &EditorTextView) {
    let word = at(NOTE, "Wort");
    let bold = after(view, word, Command::Bold);
    let unbold = after(view, view.selectedRange(), Command::Bold);
    check(
        "Fett wraps the selection and unwraps it again",
        bold.contains("Ein **Wort** hier.") && unbold == NOTE,
        format!("{bold:?} / {unbold:?}"),
    );
    let italic = after(view, word, Command::Italic);
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    check(
        "a format command is one undo step",
        italic.contains("Ein *Wort* hier.") && text_of(view) == NOTE,
        format!("{italic:?} / {:?}", text_of(view)),
    );
    let line = at(NOTE, "Zweite");
    let lines = [
        (Command::Heading2, "## Zweite Zeile"),
        (Command::BulletList, "- Zweite Zeile"),
        (Command::TaskList, "- [ ] Zweite Zeile"),
        (Command::Quote, "> Zweite Zeile"),
    ];
    let mut seen = Vec::new();
    for (command, expected) in lines {
        let on = after(view, line, command);
        let off = after(view, at(&on, "Zweite"), command);
        seen.push((on.contains(expected), off == NOTE));
    }
    check(
        "headings, lists, tasks and quotes toggle on the line",
        seen.iter().all(|(on, off)| *on && *off),
        format!("{seen:?}"),
    );
    let block = after(view, at(NOTE, "Zweite Zeile"), Command::CodeBlock);
    check(
        "Codeblock puts the line between fences",
        block.contains("```\nZweite Zeile\n```"),
        block.clone(),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    view.setSelectedRange(at(NOTE, "Wort"));
    link_with(view, "https://example.org");
    let linked = text_of(view);
    check(
        "Link … turns the selection into a link",
        linked.contains("[Wort](https://example.org)"),
        linked,
    );
}

fn pasting(_root: &Path, view: &EditorTextView) {
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    let html = "<p><b>fett</b> und <i>kursiv</i> aus dem Netz</p>";
    let html_type = constant!(NSPasteboardTypeHTML);
    pasteboard.clearContents();
    pasteboard.setData_forType(Some(&NSData::with_bytes(html.as_bytes())), html_type);
    let end = utf16_len(&text_of(view));
    view.setSelectedRange(NSRange::new(end, 0));
    let taken = paste_from(view, &pasteboard);
    let pasted = text_of(view);
    let plain = NSPasteboard::pasteboardWithUniqueName();
    let string_type = constant!(NSPasteboardTypeString);
    plain.clearContents();
    plain.setString_forType(&ns("**bleibt**"), string_type);
    let plain_taken = paste_from(view, &plain);
    check(
        "HTML on the pasteboard goes in as Markdown, plain text is left to the plain paste",
        taken && pasted.contains("**fett** und *kursiv* aus dem Netz") && !plain_taken,
        pasted,
    );
    pasteboard.clearContents();
    plain.clearContents();
}

fn menus(root: &Path) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let menu = NSApplication::sharedApplication(mtm).mainMenu();
    let format = menu
        .as_ref()
        .and_then(|menu| menu.itemWithTitle(&ns(t("Format"))))
        .and_then(|item| item.submenu());
    let bold = format.as_ref().and_then(|menu| menu.itemWithTitle(&ns(t("Fett"))));
    let key = bold.map(|item| item.keyEquivalent().to_string()).unwrap_or_default();
    check(
        "the Format menu holds the commands with splat's shortcuts",
        format.is_some_and(|menu| menu.numberOfItems() >= 13) && key == "b",
        key,
    );
    let installed = objc2_app_kit::NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&ns("local.splat"))
        .is_some();
    let offered = crate::markdown::open_in_splat_item(mtm, &root.join("notiz.md")).is_some();
    let not_for_code = crate::markdown::open_in_splat_item(mtm, &root.join("main.rs")).is_none();
    check(
        "In Splat öffnen is offered for Markdown files exactly when Splat is installed",
        offered == installed && not_for_code,
        format!("installed {installed}, offered {offered}"),
    );
}

fn exports(root: PathBuf) {
    let Some(view) = editor_of(&root, "notiz.md") else {
        return finish(SCENARIO);
    };
    let out = ide_core::paths::home().join("exports");
    let _ = std::fs::create_dir_all(&out);
    for (kind, name) in [
        (Kind::Pdf, "notiz.pdf"),
        (Kind::Html, "notiz.html"),
        (Kind::Word, "notiz.docx"),
    ] {
        let Some(source) = source_of(&view) else { continue };
        export_to(source, kind, out.join(name), None, move |outcome| {
            EXPORTED.with(|done| done.borrow_mut().push((kind, outcome)));
        });
    }
    // Saved, so quitting asks nothing.
    crate::editor::save_view(&view);
    wait_until(
        || EXPORTED.with(|done| done.borrow().len() == 3),
        move |ok| {
            EXPORTED.with(|done| exported(&out, ok, &done.borrow()));
            long_export(root, out);
        },
    );
}

thread_local! {
    /// How each export ended, as it reports it.
    static EXPORTED: RefCell<Vec<(Kind, Outcome)>> = const { RefCell::new(Vec::new()) };
    /// How the last single export ended.
    static LAST: RefCell<Option<Outcome>> = const { RefCell::new(None) };
    /// Turns of the main thread while an export ran.
    static TICKS: Cell<usize> = const { Cell::new(0) };
}

fn exported(out: &Path, ok: bool, results: &[(Kind, Outcome)]) {
    let read = |name: &str| std::fs::read(out.join(name)).unwrap_or_default();
    let (pdf, html, word) = (read("notiz.pdf"), read("notiz.html"), read("notiz.docx"));
    let html = String::from_utf8_lossy(&html);
    check(
        "the document exports to PDF, HTML and Word",
        ok && results.iter().all(|(_, outcome)| *outcome == Outcome::Written)
            && pdf.starts_with(b"%PDF")
            && word.starts_with(b"PK")
            && html.contains("<h1"),
        format!("{results:?}, pdf {} bytes, docx {} bytes", pdf.len(), word.len()),
    );
    check(
        "the HTML export carries the picture of the folder inside it",
        html.contains("data:image/png;base64,"),
        html.chars().take(120).collect::<String>(),
    );
}

/// A document long enough to take a while on paper.
fn long_source(root: &Path) -> Source {
    let mut text = String::from("# Lang\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n![bild](bild.png)\n\n");
    for n in 1..=1500 {
        text.push_str(&format!(
            "Absatz {n} mit **etwas** Text, der auf viele Seiten kommt.\n\n"
        ));
    }
    Source {
        text,
        doc_dir: Some(root.to_path_buf()),
        title: "lang".to_string(),
        trusted: true,
    }
}

fn last(outcome: Outcome) {
    LAST.with(|last| *last.borrow_mut() = Some(outcome));
}

fn finished() -> Option<Outcome> {
    LAST.with(|last| last.borrow().clone())
}

/// Counts turns of the main thread while the export `number` runs.
fn tick(number: u64) {
    if export_seen::running(number) {
        TICKS.set(TICKS.get() + 1);
        main_after(Duration::from_millis(10), move || tick(number));
    }
}

fn has_sheet(root: &Path) -> bool {
    window_of(root)
        .and_then(|c| c.window())
        .is_some_and(|window| window.attachedSheet().is_some())
}

fn long_export(root: PathBuf, out: PathBuf) {
    let window = window_of(&root).and_then(|c| c.window());
    LAST.with(|last| *last.borrow_mut() = None);
    let number = export_to(
        long_source(&root),
        Kind::Pdf,
        out.join("lang.pdf"),
        window.as_deref(),
        last,
    );
    let returned_at_once = export_seen::running(number) && has_sheet(&root);
    tick(number);
    wait_until(
        || finished().is_some(),
        move |ok| {
            let pdf = std::fs::read(out.join("lang.pdf")).unwrap_or_default();
            // Beside the window pictures, for a look at the pages.
            if let Some(report) = std::env::var_os("IDE_SELFTEST_REPORT") {
                let _ = std::fs::write(PathBuf::from(report).with_file_name("markdown-export.pdf"), &pdf);
            }
            check(
                "a long PDF export leaves the window working, with a sheet until it is done",
                ok && returned_at_once
                    && TICKS.get() >= 3
                    && finished() == Some(Outcome::Written)
                    && !has_sheet(&root)
                    && pdf.starts_with(b"%PDF"),
                format!(
                    "at once {returned_at_once}, {} turns meanwhile, {:?}, {} bytes",
                    TICKS.get(),
                    finished(),
                    pdf.len()
                ),
            );
            cancelling(root, out);
        },
    );
}

fn cancelling(root: PathBuf, out: PathBuf) {
    let window = window_of(&root).and_then(|c| c.window());
    LAST.with(|last| *last.borrow_mut() = None);
    let target = out.join("lang.docx");
    let number = export_to(long_source(&root), Kind::Word, target.clone(), window.as_deref(), last);
    let had_sheet = has_sheet(&root);
    export_seen::press_sheet_button(number);
    main_after(Duration::from_secs(1), move || {
        check(
            "Abbrechen in the sheet stops an export, which writes no file",
            had_sheet
                && finished() == Some(Outcome::Cancelled)
                && !export_seen::running(number)
                && !has_sheet(&root)
                && !target.exists(),
            format!("{:?}, file {}", finished(), target.exists()),
        );
        failing(root, out);
    });
}

fn failing(root: PathBuf, out: PathBuf) {
    let Some(view) = editor_of(&root, "notiz.md") else {
        return finish(SCENARIO);
    };
    let Some(source) = source_of(&view) else {
        return finish(SCENARIO);
    };
    let window = window_of(&root).and_then(|c| c.window());
    LAST.with(|last| *last.borrow_mut() = None);
    let target = out.join("fehlt").join("notiz.html");
    let number = export_to(source, Kind::Html, target, window.as_deref(), last);
    wait_until(
        || finished().is_some(),
        move |ok| {
            let said = export_seen::sheet_text(number).unwrap_or_default();
            let reason = match finished() {
                Some(Outcome::Failed(reason)) => reason,
                _ => String::new(),
            };
            let shown = has_sheet(&root);
            export_seen::press_sheet_button(number);
            check(
                "a failed export says why in its sheet, which OK closes",
                ok && !reason.is_empty() && said == reason && shown && !has_sheet(&root),
                format!("{said:?}"),
            );
            followed(root);
        },
    );
}

fn followed(root: PathBuf) {
    let note = root.join("notiz.md");
    let shown = note.clone();
    wait_until(
        move || crate::preview::seen::text(&shown).is_some_and(|(text, _)| text.contains("fett und kursiv")),
        move |ok| {
            check(
                "the preview follows the format commands and the paste",
                ok,
                crate::preview::seen::text(&note)
                    .map(|(text, _)| text)
                    .unwrap_or_default(),
            );
            if let Some(window) = window_of(&root).and_then(|c| c.window()) {
                snapshot(&window, "markdown");
            }
            finish(SCENARIO);
        },
    );
}
