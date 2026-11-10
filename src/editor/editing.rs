//! Typing help: indentation that follows the line above, closing brackets and quotes, commenting lines out, going to
//! a line.

use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::NSTextInputClient;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSTextField};
use objc2_foundation::{NSAttributedString, NSNotFound, NSPoint, NSRange, NSRect, NSSize, NSString};
use splat_i18n::t;

use super::view::EditorTextView;
use crate::util::ns;

/// Pairs typed together; the closing one is skipped over when typed in front of itself.
const PAIRS: [(char, char); 6] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`')];

fn text_of(object: &AnyObject) -> Option<String> {
    if let Some(s) = object.downcast_ref::<NSString>() {
        return Some(s.to_string());
    }
    object
        .downcast_ref::<NSAttributedString>()
        .map(|s| s.string().to_string())
}

/// The character at UTF-16 offset `at`, if there is one.
fn char_at(view: &EditorTextView, at: usize) -> Option<char> {
    let string = view.string();
    if at >= string.length() {
        return None;
    }
    let unit = string.characterAtIndex(at);
    char::from_u32(u32::from(unit))
}

fn insert(view: &EditorTextView, text: &str) {
    // SAFETY: The inserted object is an NSString, one of the two types this method accepts.
    unsafe { view.insertText_replacementRange(&ns(text), NSRange::new(NSNotFound.cast_unsigned(), 0)) };
}

/// Closes brackets and quotes as they are typed, steps over a closing one already there, and wraps a selection in a
/// pair; whether it handled `object`.
pub fn typed(view: &EditorTextView, object: &AnyObject, replacement: NSRange) -> bool {
    if replacement.location != NSNotFound.cast_unsigned() || view.ivars().document.borrow().read_only().is_some() {
        return false;
    }
    let Some(text) = text_of(object) else { return false };
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return false;
    };
    let selection = view.selectedRange();
    let next = char_at(view, selection.location + selection.length);
    if selection.length == 0 && next == Some(c) && PAIRS.iter().any(|(_, close)| *close == c) {
        view.setSelectedRange(NSRange::new(selection.location + 1, 0));
        return true;
    }
    let Some((open, close)) = PAIRS.iter().find(|(open, _)| *open == c).copied() else {
        return false;
    };
    if selection.length > 0 {
        let selected = view.string().substringWithRange(selection).to_string();
        insert(view, &format!("{open}{selected}{close}"));
        view.setSelectedRange(NSRange::new(selection.location + 1, selection.length));
        return true;
    }
    let free_after = next.is_none_or(|n| n.is_whitespace() || ")]},;".contains(n));
    let previous = selection.location.checked_sub(1).and_then(|at| char_at(view, at));
    let quote = open == close;
    if !free_after || (quote && previous.is_some_and(|p| p.is_alphanumeric() || p == open)) {
        return false;
    }
    insert(view, &format!("{open}{close}"));
    view.setSelectedRange(NSRange::new(selection.location + 1, 0));
    true
}

/// The line around UTF-16 offset `at`: its start and its text up to `at`.
fn line_before(view: &EditorTextView, at: usize) -> (usize, String) {
    let string = view.string();
    let line = string.lineRangeForRange(NSRange::new(at, 0));
    let before = string
        .substringWithRange(NSRange::new(line.location, at - line.location))
        .to_string();
    (line.location, before)
}

/// A new line indented like the one above, one level deeper after an opening bracket or colon; between a bracket pair
/// the closing one moves to a line of its own.
pub fn newline(view: &EditorTextView) {
    let selection = view.selectedRange();
    let (_, before) = line_before(view, selection.location);
    let indent: String = before.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let ending = super::indent::newline(view).as_str().to_string();
    let unit = view.ivars().indent.borrow().clone();
    let opens = before.trim_end().ends_with(['{', '(', '[', ':']);
    let next = char_at(view, selection.location + selection.length);
    let closes = next.is_some_and(|n| "}])".contains(n));
    if opens && closes {
        insert(view, &format!("{ending}{indent}{unit}{ending}{indent}"));
        let back = indent.encode_utf16().count() + ending.encode_utf16().count();
        let at = view.selectedRange().location - back;
        view.setSelectedRange(NSRange::new(at, 0));
    } else if opens {
        insert(view, &format!("{ending}{indent}{unit}"));
    } else {
        insert(view, &format!("{ending}{indent}"));
    }
}

/// Inserts one level of indentation, or indents every selected line.
pub fn tab(view: &EditorTextView) {
    let unit = view.ivars().indent.borrow().clone();
    let selection = view.selectedRange();
    let string = view.string();
    let lines = string.lineRangeForRange(selection);
    let spans_lines = string.substringWithRange(selection).to_string().contains('\n');
    if !spans_lines {
        insert(view, &unit);
        return;
    }
    let block = string.substringWithRange(lines).to_string();
    let indented: String = block
        .split_inclusive('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{unit}{line}")
            }
        })
        .collect();
    replace(view, lines, &indented);
}

/// The marker that starts a line comment in the document's language.
fn comment_marker(view: &EditorTextView) -> Option<&'static str> {
    let language = view.document().language()?;
    match language.name() {
        "python" | "bash" | "yaml" | "toml" | "ruby" => Some("#"),
        "sql" => Some("--"),
        "html" | "css" | "mermaid" | "tex" | "markdown" => None,
        _ => Some("//"),
    }
}

/// Comments the selected lines out, or in again when all of them are comments.
pub fn toggle_comment(view: &EditorTextView) {
    let Some(marker) = comment_marker(view) else { return };
    let string = view.string();
    let lines = string.lineRangeForRange(view.selectedRange());
    let block = string.substringWithRange(lines).to_string();
    let content: Vec<&str> = block.split_inclusive('\n').collect();
    let commented = content
        .iter()
        .filter(|line| !line.trim().is_empty())
        .all(|line| line.trim_start().starts_with(marker));
    let indent = content
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    let changed: String = content
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                (*line).to_string()
            } else if commented {
                let start = line.len() - line.trim_start().len();
                let rest = &line[start + marker.len()..];
                format!("{}{}", &line[..start], rest.strip_prefix(' ').unwrap_or(rest))
            } else {
                format!("{}{marker} {}", &line[..indent], &line[indent..])
            }
        })
        .collect();
    replace(view, lines, &changed);
}

/// Replaces `range` like typing would, as one undoable step.
fn replace(view: &EditorTextView, range: NSRange, text: &str) {
    if view.shouldChangeTextInRange_replacementString(range, Some(&ns(text))) {
        if let Some(storage) = view.storage() {
            storage.replaceCharactersInRange_withString(range, &ns(text));
        }
        view.didChangeText();
        view.setSelectedRange(NSRange::new(range.location, text.encode_utf16().count()));
    }
}

/// Asks for a line number and puts the cursor at its start.
pub fn go_to_line(view: &EditorTextView) {
    let mtm = view.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Gehe zu Zeile")));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(120.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    if alert.runModal() != NSAlertFirstButtonReturn {
        return;
    }
    let Ok(line) = field.stringValue().to_string().trim().parse::<usize>() else {
        return;
    };
    let count = view.document().text().line_count();
    if let Some(start) = view.line_start(line.clamp(1, count.max(1)) - 1) {
        view.setSelectedRange(NSRange::new(start, 0));
        view.scrollRangeToVisible(NSRange::new(start, 0));
    }
}
