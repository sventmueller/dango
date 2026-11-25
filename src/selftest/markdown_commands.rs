//! The `markdown-commands` scenario: the Format commands the `markdown` scenario leaves out change the source as
//! their names say, a heading of the same level toggles off; Link asks for the address and inserts nothing when
//! cancelled; the menu items of a Markdown document are enabled; HTML with a picture pastes as a block with the
//! picture saved beside the document; the HTML export offers the document's folder and name and writes there; opening
//! in Splat saves the document first and, when it cannot, leaves it here.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSMenuItem, NSModalResponseOK, NSPasteboard,
    NSPasteboardTypeHTML, NSRunningApplication, NSSavePanel, NSTextField, NSView,
};
use objc2_foundation::{NSData, NSRange};

use super::helpers::{self, editor, text_of, utf16_len, window_of};
use super::{check, finish, give_up};
use crate::editor::EditorTextView;
use crate::markdown::selftest::{Command, paste_from, saved_for_splat};
use crate::util::{constant, ns};

const NAME: &str = "markdown-commands";
/// A picture of one pixel.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

pub(crate) fn run() {
    helpers::open_project(NAME, "markdown-commands", &[("doc/note.md", "word\n")], |_| {
        let root = helpers::root("markdown-commands");
        helpers::wait_for_editor(&root.clone(), &root.join("doc/note.md"), move |view| {
            on_editor(view, root);
        });
    });
}

fn on_editor(view: Option<Retained<EditorTextView>>, root: PathBuf) {
    let Some(view) = view else {
        return give_up(NAME, "the Markdown file opens in an editor");
    };
    format(&view);
    link(&view);
    validate(&view);
    paste(&view, &root);
    export(&view, root);
}

/// Replaces the text of `view` with `text` and selects `word` in it.
fn set(view: &EditorTextView, text: &str, word: &str) {
    helpers::type_at(view, NSRange::new(0, utf16_len(&text_of(view))), text);
    let at = text.find(word).map_or(0, |at| utf16_len(&text[..at]));
    view.setSelectedRange(NSRange::new(at, utf16_len(word)));
}

fn perform(view: &EditorTextView, command: Command) -> String {
    crate::markdown::perform(view, command as isize);
    text_of(view)
}

fn format(view: &EditorTextView) {
    let mut seen = Vec::new();
    for (command, wanted) in [
        (Command::Strikethrough, "~~word~~"),
        (Command::Code, "`word`"),
        (Command::Heading1, "# word"),
        (Command::Heading3, "### word"),
        (Command::NumberedList, "1. word"),
    ] {
        set(view, "word", "word");
        let text = perform(view, command);
        if text.trim_end() != wanted {
            seen.push(format!("{command:?}: {text:?}"));
        }
    }
    check(
        "strikethrough, code, headings 1 and 3 and the numbered list change the source as their names say",
        seen.is_empty(),
        seen.join("; "),
    );
    set(view, "# word", "word");
    let off = perform(view, Command::Heading1);
    check(
        "a heading of the level asked for becomes plain text again",
        off.trim_end() == "word",
        off,
    );
}

/// The alert's text field the address goes into.
fn address_field() -> Option<Retained<NSTextField>> {
    let mtm = MainThreadMarker::new()?;
    let alert = NSApplication::sharedApplication(mtm).modalWindow()?;
    let mut found = None;
    let content = alert.contentView()?;
    find_editable(&content, &mut found);
    found
}

fn find_editable(view: &NSView, found: &mut Option<Retained<NSTextField>>) {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        *found = Some(field.retain());
        return;
    }
    for child in view.subviews().iter() {
        find_editable(&child, found);
    }
}

fn link(view: &EditorTextView) {
    set(view, "see word", "word");
    helpers::when_alert_shows(None, |_| {
        if let Some(field) = address_field() {
            field.setStringValue(&ns("  https://example.org/docs  "));
        }
        helpers::answer_alert(NSAlertFirstButtonReturn);
    });
    let linked = perform(view, Command::Link);
    check(
        "Link asks for the address and makes the selection the link's text",
        linked.trim_end() == "see [word](https://example.org/docs)",
        linked,
    );
    set(view, "see word", "word");
    helpers::answer_alert_when_shown(None, NSAlertSecondButtonReturn);
    let cancelled = perform(view, Command::Link);
    check(
        "a cancelled Link changes nothing",
        cancelled.trim_end() == "see word",
        cancelled,
    );
}

fn validate(view: &EditorTextView) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let enabled = |tag: isize| {
        let item = NSMenuItem::new(mtm);
        // SAFETY: The item is only asked about, never shown or clicked.
        unsafe { item.setAction(Some(sel!(ideMarkdown:))) };
        item.setTag(tag);
        crate::markdown::validate(view, &item)
    };
    let splat = view
        .document()
        .path()
        .is_some_and(|path| crate::markdown::open_in_splat_item(mtm, path).is_some());
    let answers = [
        enabled(Command::PastePlain as isize),
        enabled(Command::ExportPdf as isize),
        enabled(Command::ExportWord as isize),
        enabled(Command::OpenInSplat as isize),
        enabled(999),
    ];
    let preview = crate::preview::validate(view, Some(sel!(ideShowPreview:)));
    check(
        "a Markdown document enables plain pasting, the exports, its preview and, with Splat there, In Splat öffnen",
        answers == [Some(true), Some(true), Some(true), Some(splat), Some(false)] && preview == Some(true),
        format!("{answers:?}, Splat installed {splat}, preview {preview:?}"),
    );
}

fn paste(view: &EditorTextView, root: &Path) {
    let html = format!("<h2>Pasted</h2><p>with <img src=\"data:image/png;base64,{PNG}\"></p>");
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    pasteboard.setData_forType(
        Some(&NSData::with_bytes(html.as_bytes())),
        constant!(NSPasteboardTypeHTML),
    );
    set(view, "before\n", "");
    view.setSelectedRange(NSRange::new(utf16_len("before\n"), 0));
    let taken = paste_from(view, &pasteboard);
    let text = text_of(view);
    let picture = root.join("doc").join(format!("{}.png", splat_i18n::t("Bild")));
    check(
        "HTML with a picture pastes as Markdown blocks, the picture saved beside the document and linked",
        taken && text.contains("## Pasted") && text.contains(".png)") && picture.is_file(),
        format!("{text:?}, picture there {}", picture.is_file()),
    );
    pasteboard.clearContents();
}

/// File ▸ Export ▸ HTML: the save panel offers the document's folder and name, and the export lands there.
fn export(view: &EditorTextView, root: PathBuf) {
    let offered = std::rc::Rc::new(std::cell::RefCell::new(None));
    let seen = std::rc::Rc::clone(&offered);
    helpers::when_alert_shows(None, move |window| {
        if let Some(panel) = window.downcast_ref::<NSSavePanel>() {
            let folder = panel.directoryURL().and_then(|url| url.path()).map(|p| p.to_string());
            *seen.borrow_mut() = Some((panel.nameFieldStringValue().to_string(), folder));
            helpers::answer_alert(NSModalResponseOK);
        }
    });
    crate::markdown::perform(view, Command::ExportHtml as isize);
    let offered = offered.borrow().clone();
    let file = root.join("doc/note.html");
    let probe = file.clone();
    helpers::wait_until(
        move || probe.is_file(),
        move |written| {
            let folder = root.join("doc").canonicalize().ok();
            let in_folder = offered
                .as_ref()
                .and_then(|(_, dir)| dir.as_deref())
                .is_some_and(|dir| std::path::Path::new(dir).canonicalize().ok() == folder);
            let html = std::fs::read_to_string(&file).unwrap_or_default();
            check(
                "Export as HTML offers the document's folder and name, and writes the document there",
                written
                    && in_folder
                    && offered.as_ref().is_some_and(|(name, _)| name == "note.html")
                    && html.contains("Pasted"),
                format!("offered {offered:?}, written {written}"),
            );
            let closed = || {
                MainThreadMarker::new().is_some_and(|mtm| NSApplication::sharedApplication(mtm).modalWindow().is_none())
            };
            helpers::wait_until(closed, move |_| splat(root));
        },
    );
}

fn running_splats() -> usize {
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&ns("local.splat")).count()
}

fn splat(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(view) = editor(&controller, "note.md") else {
        return give_up(NAME, "note.md opens in an editor");
    };
    let path = view.document().path().map(Path::to_path_buf).unwrap_or_default();
    set(&view, "saved for Splat\n", "");
    let saved = saved_for_splat(&path);
    check(
        "before Splat opens a document, an editor with unsaved changes of it saves them",
        saved && !view.document().is_dirty() && std::fs::read_to_string(&path).is_ok_and(|t| t == "saved for Splat\n"),
        format!("saved {saved}"),
    );
    set(&view, "stays here\n", "");
    let folder = root.join("doc");
    let locked = |mode: u32| {
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode & 0o666));
        let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(mode));
    };
    locked(0o555);
    let failed = splat_i18n::t("Sichern fehlgeschlagen").to_string();
    let asked = helpers::answer_alert_saying(failed.clone(), NSAlertFirstButtonReturn);
    let refused = !saved_for_splat(&path);
    let splats = running_splats();
    let mut opened_here = false;
    if refused && asked.get() {
        let item = MainThreadMarker::new().and_then(|mtm| crate::markdown::open_in_splat_item(mtm, &path));
        if let Some(item) = item {
            let again = helpers::answer_alert_saying(failed, NSAlertFirstButtonReturn);
            let app = NSApplication::sharedApplication(controller.mtm());
            let target = item.target();
            if let Some(action) = item.action() {
                // SAFETY: The item's own action and target, with the item as the sender, as a click sends them.
                unsafe { app.sendAction_to_from(action, target.as_deref(), Some(&item)) };
            }
            opened_here = crate::editor::run_until(move || again.get(), std::time::Duration::from_secs(10));
        }
    }
    locked(0o755);
    check(
        "a document that cannot be saved stays here: In Splat öffnen tries to save it and opens nothing",
        refused && asked.get() && opened_here && view.document().is_dirty() && running_splats() == splats,
        format!("refused {refused}, asked {}, tried again {opened_here}", asked.get()),
    );
    // After the refused saves the editor may ask before it overwrites; without an alert an answer does nothing.
    helpers::answer_alert_when_shown(None, NSAlertFirstButtonReturn);
    helpers::answer_alert_when_shown(None, NSAlertFirstButtonReturn);
    crate::editor::save(&view, false);
    finish(NAME);
}
