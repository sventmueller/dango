//! What a Markdown document offers beyond the code editor (E59): its preview opening beside it, the format commands of
//! splat's toolbar, pasting from Word or the web as Markdown, exports to PDF, HTML and Word, and opening it in Splat.
//! The commands go through the editor's one action `ideMarkdown:`, told apart by the menu item's tag.

mod format;
mod paste;
mod splat_app;

use std::ops::Range;
use std::path::PathBuf;

use ide_core::settings::{Settings, keys};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::NSRange;
use splat_i18n::t;
use splat_markdown::commands::Edit;
use splat_markdown::text::Byte;

use crate::editor::EditorTextView;
use crate::util::{ns, separator, submenu, tagged};
use ide_ui_export as export;
pub use splat_app::open_in_splat_item;

/// A command of the menus, by the tag of its menu item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Bold = 1,
    Italic,
    Strikethrough,
    Code,
    Link,
    Heading1,
    Heading2,
    Heading3,
    BulletList,
    NumberedList,
    TaskList,
    Quote,
    CodeBlock,
    PastePlain,
    ExportPdf,
    ExportHtml,
    ExportWord,
    OpenInSplat,
}

const ALL: [Command; 18] = [
    Command::Bold,
    Command::Italic,
    Command::Strikethrough,
    Command::Code,
    Command::Link,
    Command::Heading1,
    Command::Heading2,
    Command::Heading3,
    Command::BulletList,
    Command::NumberedList,
    Command::TaskList,
    Command::Quote,
    Command::CodeBlock,
    Command::PastePlain,
    Command::ExportPdf,
    Command::ExportHtml,
    Command::ExportWord,
    Command::OpenInSplat,
];

impl Command {
    fn of_tag(tag: isize) -> Option<Command> {
        ALL.into_iter().find(|command| *command as isize == tag)
    }

    /// The name of its shortcut in the keymap (E26), for those that have one.
    fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Command::Bold => "format.bold",
            Command::Italic => "format.italic",
            Command::Strikethrough => "format.strikethrough",
            Command::Code => "format.code",
            Command::Link => "format.link",
            Command::Heading1 => "format.heading1",
            Command::Heading2 => "format.heading2",
            Command::Heading3 => "format.heading3",
            Command::PastePlain => "edit.pastePlain",
            _ => return None,
        })
    }

    fn title(self) -> &'static str {
        match self {
            Command::Bold => t("Fett"),
            Command::Italic => t("Kursiv"),
            Command::Strikethrough => t("Durchgestrichen"),
            Command::Code => t("Code"),
            Command::Link => t("Link …"),
            Command::Heading1 => t("Überschrift 1"),
            Command::Heading2 => t("Überschrift 2"),
            Command::Heading3 => t("Überschrift 3"),
            Command::BulletList => t("Aufzählung"),
            Command::NumberedList => t("Nummerierte Liste"),
            Command::TaskList => t("Aufgabe"),
            Command::Quote => t("Zitat"),
            Command::CodeBlock => t("Codeblock"),
            Command::PastePlain => t("Als reinen Text einsetzen"),
            Command::ExportPdf => t("PDF …"),
            Command::ExportHtml => t("HTML …"),
            Command::ExportWord => t("Word …"),
            Command::OpenInSplat => t("In Splat öffnen"),
        }
    }

    fn item(self, mtm: MainThreadMarker, shortcut: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
        let key = self.shortcut().map(shortcut).unwrap_or_default();
        tagged(mtm, self.title(), sel!(ideMarkdown:), &key, self as isize)
    }
}

/// The Format menu, its shortcuts from the keymap.
pub fn format_menu(mtm: MainThreadMarker, shortcut: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
    use Command::*;
    let groups: [&[Command]; 4] = [
        &[Bold, Italic, Strikethrough, Code],
        &[Link],
        &[Heading1, Heading2, Heading3],
        &[BulletList, NumberedList, TaskList, Quote, CodeBlock],
    ];
    let mut items = Vec::new();
    for (n, group) in groups.iter().enumerate() {
        if n > 0 {
            items.push(separator(mtm));
        }
        items.extend(group.iter().map(|command| command.item(mtm, shortcut)));
    }
    submenu(mtm, t("Format"), items).0
}

/// The File menu's entries for Markdown documents: the exports and, where Splat is installed, opening it there.
pub fn file_items(mtm: MainThreadMarker) -> Vec<Retained<NSMenuItem>> {
    let no_key = |_: &str| String::new();
    let exports = [Command::ExportPdf, Command::ExportHtml, Command::ExportWord]
        .iter()
        .map(|command| command.item(mtm, &no_key))
        .collect();
    let open = Command::OpenInSplat.item(mtm, &no_key);
    open.setHidden(splat_app::location().is_none());
    vec![submenu(mtm, t("Exportieren"), exports).0, open]
}

/// The Edit menu's entry that pastes text without its formatting.
pub fn paste_plain_item(mtm: MainThreadMarker, shortcut: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
    Command::PastePlain.item(mtm, shortcut)
}

/// Runs the command of the menu item tagged `tag` on the editor `view`.
pub fn perform(view: &EditorTextView, tag: isize) {
    let Some(command) = Command::of_tag(tag) else { return };
    match command {
        Command::PastePlain => paste::plain(view),
        Command::ExportPdf => export_as(view, export::Kind::Pdf),
        Command::ExportHtml => export_as(view, export::Kind::Html),
        Command::ExportWord => export_as(view, export::Kind::Word),
        Command::OpenInSplat => {
            if let Some(path) = view.document().path() {
                splat_app::open(view.mtm(), path);
            }
        }
        command => format::run(view, command),
    }
}

/// Asks where the Markdown document of `view` goes as `kind`, then writes it there.
fn export_as(view: &EditorTextView, kind: export::Kind) {
    if let Some(source) = source_of(view) {
        export::ask(view.mtm(), view.window().as_deref(), source, kind);
    }
}

/// What an export of the document `view` shows makes a file of.
pub fn source_of(view: &EditorTextView) -> Option<export::Source> {
    let document = view.document();
    let path = document.path()?;
    Some(export::Source {
        text: document.text().to_string(),
        doc_dir: path.parent().map(std::path::Path::to_path_buf),
        title: path.file_stem()?.to_string_lossy().into_owned(),
        trusted: view.controller().is_some_and(|c| c.is_trusted()),
    })
}

/// Whether the menu item of an `ideMarkdown:` command applies to the editor `view`; `None` for other items.
pub fn validate(view: &EditorTextView, item: &NSMenuItem) -> Option<bool> {
    (item.action() == Some(sel!(ideMarkdown:))).then(|| match Command::of_tag(item.tag()) {
        Some(Command::PastePlain) => view.isEditable(),
        Some(Command::OpenInSplat) => is_markdown(view) && splat_app::location().is_some(),
        Some(Command::ExportPdf | Command::ExportHtml | Command::ExportWord) => is_markdown(view),
        Some(_) => is_markdown(view) && view.isEditable(),
        None => false,
    })
}

/// ⌘V: in a Markdown document, rich text from Word or the web goes in as Markdown; whether the paste was taken.
pub fn paste(view: &EditorTextView) -> bool {
    is_markdown(view) && paste::rich(view, &objc2_app_kit::NSPasteboard::generalPasteboard())
}

/// An editor opened `view` at the user's request: a Markdown file gets its preview beside it, if the user wants that.
pub fn opened(view: &EditorTextView) {
    if is_markdown(view) && Settings::load_user().get(&keys::MARKDOWN_PREVIEW) {
        crate::preview::open_beside(view, true);
    }
}

/// The editor `view` closes: the preview that opened with it goes too.
pub fn closing(view: &EditorTextView) {
    crate::preview::close_beside(view, true);
}

fn is_markdown(view: &EditorTextView) -> bool {
    view.document().path().is_some_and(crate::preview::is_markdown)
}

/// The folder of the document `view` shows, where relative links and pictures start.
fn doc_dir(view: &EditorTextView) -> Option<PathBuf> {
    view.document().path()?.parent().map(PathBuf::from)
}

/// The text of `view` and its selection, in bytes.
fn text_and_selection(view: &EditorTextView) -> (String, Range<Byte>) {
    let document = view.document();
    let text = document.text();
    let selected = view.selectedRange();
    let byte = |utf16: usize| Byte::new(text.utf16_to_byte(utf16).unwrap_or_else(|_| text.to_string().len()));
    let range = byte(selected.location)..byte(selected.location + selected.length);
    (text.to_string(), range)
}

/// The UTF-16 offset of byte `at` of `text`.
fn utf16(text: &str, at: Byte) -> usize {
    text.get(..at.get())
        .map_or_else(|| text.encode_utf16().count(), |s| s.encode_utf16().count())
}

/// Makes `edit` in `view` like typing does, as one undo step named `name`, and selects what it says.
fn apply(view: &EditorTextView, edit: &Edit, name: &str) {
    let (text, _) = text_and_selection(view);
    let start = utf16(&text, edit.range.start);
    let range = NSRange::new(start, utf16(&text, edit.range.end) - start);
    let replacement = ns(&edit.replacement);
    let replaced = std::cell::Cell::new(false);
    crate::editor::undo_step(view, || {
        if view.shouldChangeTextInRange_replacementString(range, Some(&replacement)) {
            if let Some(storage) = view.storage() {
                storage.replaceCharactersInRange_withString(range, &replacement);
            }
            view.didChangeText();
            replaced.set(true);
        }
    });
    if !replaced.get() {
        return;
    }
    if let Some(undo) = view.undoManager() {
        undo.setActionName(&ns(name));
    }
    let edited = edit.applied_to(&text);
    let from = utf16(&edited, edit.selection.start);
    view.setSelectedRange(NSRange::new(from, utf16(&edited, edit.selection.end) - from));
    view.scrollRangeToVisible(view.selectedRange());
}

/// What the self-test calls directly, without menus, panels or the general pasteboard.
#[cfg(feature = "selftest")]
pub mod selftest {
    pub use super::Command;
    pub use super::export::seen as export_seen;
    pub use super::export::{Kind, Outcome, Source, start as export_to};
    pub use super::format::link_with;
    pub use super::paste::rich as paste_from;
    pub use super::source_of;
}
