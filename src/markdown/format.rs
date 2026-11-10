//! The format commands of splat's toolbar as source edits: a style toggles on the selection or the word at the caret,
//! a block kind toggles on the selected lines; each is one undo step.

use objc2::MainThreadOnly;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSPasteboard, NSPasteboardTypeString, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;
use splat_markdown::commands::{self, Edit, InlineStyle, ListStyle};
use splat_markdown::text::TextAt;

use super::{Command, apply, text_and_selection};
use crate::editor::EditorTextView;
use crate::util::{constant, ns};

pub fn run(view: &EditorTextView, command: Command) {
    if command == Command::Link {
        if let Some(url) = ask_link(view) {
            link_with(view, &url);
        }
        return;
    }
    let (text, sel) = text_and_selection(view);
    let doc = splat_markdown::reader::parse(&text);
    let inline = |style| commands::toggle_inline(&doc, sel.clone(), style);
    let heading = |level: u8| {
        let current = commands::active_at(&doc, sel.start).heading;
        commands::set_heading(&doc, sel.clone(), if current == level { 0 } else { level })
    };
    let edit: Edit = match command {
        Command::Bold => inline(InlineStyle::Bold),
        Command::Italic => inline(InlineStyle::Italic),
        Command::Strikethrough => inline(InlineStyle::Strikethrough),
        Command::Code => inline(InlineStyle::Code),
        Command::Heading1 => heading(1),
        Command::Heading2 => heading(2),
        Command::Heading3 => heading(3),
        Command::BulletList => commands::toggle_list(&text, sel, ListStyle::Bullet),
        Command::NumberedList => commands::toggle_list(&text, sel, ListStyle::Numbered),
        Command::TaskList => commands::toggle_list(&text, sel, ListStyle::Task),
        Command::Quote => commands::toggle_quote(&text, sel),
        Command::CodeBlock => commands::code_block_language(&doc, sel, ""),
        _ => return,
    };
    apply(view, &edit, command.title().trim_end_matches(" …"));
}

/// Inserts a link to `url` at the selection, which becomes its text; without a selection the address is the text.
pub fn link_with(view: &EditorTextView, url: &str) {
    let (text, sel) = text_and_selection(view);
    let selected = text.at(sel.clone()).to_string();
    let label = if selected.trim().is_empty() {
        url
    } else {
        selected.as_str()
    };
    let edit = commands::insert_link(&text, sel, url, label);
    apply(view, &edit, t("Link einfügen"));
}

/// Asks for the address of a link, offering one on the pasteboard; `None` when the user cancels.
fn ask_link(view: &EditorTextView) -> Option<String> {
    let mtm = view.mtm();
    let offered = NSPasteboard::generalPasteboard()
        .stringForType(constant!(NSPasteboardTypeString))
        .map(|s| s.to_string().trim().to_string())
        .filter(|s| s.contains("://") && !s.contains(char::is_whitespace))
        .unwrap_or_default();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Link einfügen")));
    alert.setInformativeText(&ns(t("Adresse des Links:")));
    alert.addButtonWithTitle(&ns(t("Einfügen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(&offered), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    if alert.runModal() != NSAlertFirstButtonReturn {
        return None;
    }
    let url = field.stringValue().to_string().trim().to_string();
    (!url.is_empty()).then_some(url)
}
