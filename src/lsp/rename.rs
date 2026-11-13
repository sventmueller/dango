//! Renaming a symbol across the files of the folder, and formatting a document, on request or when it is saved.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use block2::RcBlock;
use ide_core::settings::{Settings, keys};
use ide_lsp::Error;
use ide_lsp::lsp_types::{FormattingOptions, PrepareRenameResponse, TextEdit};
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSModalResponse, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::{server_of, text};
use crate::editor::{self, EditorTextView};
use crate::util::{guarded, main_after, main_soon, ns};

/// How long saving waits for the server's formatting before it saves the text as it is.
const FORMAT_WAIT: Duration = Duration::from_secs(2);

/// The word of letters, digits and underscores around UTF-16 offset `index` of `text`.
fn word_at(text: &str, index: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let is_word = |u: &u16| char::from_u32(u32::from(*u)).is_some_and(|c| c.is_alphanumeric() || c == '_');
    let start = units[..index.min(units.len())]
        .iter()
        .rposition(|u| !is_word(u))
        .map_or(0, |p| p + 1);
    let end = units[index.min(units.len())..]
        .iter()
        .position(|u| !is_word(u))
        .map_or(units.len(), |p| index + p);
    String::from_utf16_lossy(&units[start..end.max(start)])
}

/// Asks for a new name for the symbol at the cursor, after the server said it can be renamed.
pub fn rename(view: &EditorTextView) {
    let Some((server, uri)) = server_of(view) else { return };
    let index = view.selectedRange().location;
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.ivars().lsp.borrow().serial;
    let started = server.prepare_rename(&uri, position, move |answer| {
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let current = match answer {
                Ok(Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. })) => placeholder,
                Ok(Some(PrepareRenameResponse::Range(range))) => {
                    let range = text::range(view.document().text(), range);
                    let range = objc2_foundation::NSRange::new(range.start, range.end - range.start);
                    view.string().substringWithRange(range).to_string()
                }
                Ok(Some(PrepareRenameResponse::DefaultBehavior { .. })) => word_at(&view.string().to_string(), index),
                Ok(None) | Err(_) => {
                    objc2_app_kit::NSBeep();
                    return;
                }
            };
            ask_name(&view, index, &current);
        });
    });
    if let Err(Error::Unsupported(_)) = started {
        let current = word_at(&view.string().to_string(), index);
        ask_name(view, index, &current);
    }
}

fn ask_name(view: &EditorTextView, index: usize, current: &str) {
    let Some(window) = view.window() else { return };
    let mtm = view.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf("„{}“ umbenennen", &[current])));
    alert.setInformativeText(&ns(t("Der neue Name gilt in allen Dateien des Ordners.")));
    alert.addButtonWithTitle(&ns(t("Umbenennen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(280.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let serial = view.ivars().lsp.borrow().serial;
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            let name = field.stringValue().to_string();
            if response == NSAlertFirstButtonReturn
                && !name.trim().is_empty()
                && let Some(view) = editor::find_editor(serial)
            {
                rename_to(&view, index, name.trim());
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
}

/// Renames the symbol at UTF-16 offset `index` to `name` in every file the server names.
pub fn rename_to(view: &EditorTextView, index: usize, name: &str) {
    let Some((server, uri)) = server_of(view) else { return };
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.ivars().lsp.borrow().serial;
    let _ = server.rename(&uri, position, name, move |answer| {
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let Some(folder) = view.controller().and_then(|c| c.project_root()) else {
                return;
            };
            let problem = match answer {
                Ok(Some(edit)) => {
                    return super::edits::apply_workspace_edit(&folder, &edit, |applied| {
                        if let (Err(failure), Some(mtm)) = (applied, objc2::MainThreadMarker::new()) {
                            editor::show_alert(mtm, t("Umbenennen fehlgeschlagen"), &failure.message);
                        }
                    });
                }
                Ok(None) => t("Der Sprachserver nennt nichts umzubenennen.").to_string(),
                Err(error) => error.to_string(),
            };
            editor::show_alert(view.mtm(), t("Umbenennen fehlgeschlagen"), &problem);
        });
    });
}

fn options(view: &EditorTextView) -> FormattingOptions {
    let (tab_size, insert_spaces) = editor::formatting_options(view);
    FormattingOptions {
        tab_size,
        insert_spaces,
        ..FormattingOptions::default()
    }
}

/// What runs after formatting, such as saving.
pub type Then = Box<dyn FnOnce(&EditorTextView)>;

/// Formatting requests waiting for their answer, by the editor's serial.
struct Pending {
    id: u64,
    revision: u64,
    then: Option<Then>,
}

/// The format-on-save requests that run, by editor.
#[derive(Default)]
pub struct Formatting {
    formatting: RefCell<HashMap<u64, Pending>>,
}

fn with_formatting<R>(f: impl FnOnce(&RefCell<HashMap<u64, Pending>>) -> R) -> R {
    f(&crate::context::app().formatting.formatting)
}

/// Formats the document with its server, as one undoable step; `then`, such as saving, runs afterwards, also when
/// nothing changed or the server did not answer within `FORMAT_WAIT`.
pub fn format(view: &EditorTextView, then: Option<Then>) {
    let Some((server, uri)) = server_of(view) else {
        if let Some(then) = then {
            then(view);
        }
        return;
    };
    let serial = view.ivars().lsp.borrow().serial;
    let id = super::new_serial();
    let started = server.format(&uri, options(view), move |answer| {
        main_soon(move || finished(serial, id, Some(answer)));
    });
    match started {
        Ok(_) => {
            let revision = view.document().revision();
            let waits = then.is_some();
            with_formatting(|all| all.borrow_mut().insert(serial, Pending { id, revision, then }));
            if waits {
                main_after(FORMAT_WAIT, move || finished(serial, id, None));
            }
        }
        Err(error) => match then {
            Some(then) => then(view),
            None => editor::show_alert(view.mtm(), t("Formatieren fehlgeschlagen"), &error.to_string()),
        },
    }
}

/// The answer to formatting request `id`, or `None` when the wait ran out first.
fn finished(serial: u64, id: u64, answer: Option<Result<Vec<TextEdit>, Error>>) {
    let pending = with_formatting(|all| {
        let mut all = all.borrow_mut();
        if all.get(&serial).is_some_and(|p| p.id == id) {
            all.remove(&serial)
        } else {
            None
        }
    });
    let (Some(pending), Some(view)) = (pending, editor::find_editor(serial)) else {
        return;
    };
    match answer {
        Some(Ok(edits)) if view.document().revision() == pending.revision && !edits.is_empty() => {
            let ranges = super::edits::ranges(&view, &edits);
            super::edits::apply_text_edits(&view, ranges);
        }
        Some(Err(error)) => crate::diag::report("formatting", error),
        _ => {}
    }
    if let Some(then) = pending.then {
        then(&view);
    }
}

/// Saves `view` as ⌘S does: formatted first when the setting asks for it and the server can.
pub fn save(view: &EditorTextView) {
    let wanted = view
        .controller()
        .and_then(|c| c.project_root())
        .is_some_and(|root| Settings::load_user().in_project(&root).get(&keys::FORMAT_ON_SAVE));
    let can = server_of(view).is_some_and(|(server, _)| {
        server
            .capabilities()
            .is_some_and(|c| c.document_formatting_provider.is_some())
    });
    if wanted && can {
        format(view, Some(Box::new(|view| editor::save(view, false))));
    } else {
        editor::save(view, false);
    }
}

#[cfg(test)]
mod tests {
    use super::word_at;

    #[test]
    fn the_word_at_the_cursor_is_what_renaming_offers_to_change() {
        let text = "let total_2 = önce(x);";
        assert_eq!(word_at(text, 6), "total_2");
        assert_eq!(word_at(text, 4), "total_2", "at the word's start");
        assert_eq!(word_at(text, 11), "total_2", "right after its end");
        assert_eq!(word_at(text, 15), "önce");
        assert_eq!(word_at(text, 12), "");
        assert_eq!(word_at("😀name", 2), "name", "an emoji is no part of a word");
        assert_eq!(word_at("abc", 99), "abc");
    }
}
