//! Renaming a symbol across the files of the folder, and formatting a document, on request or when it is saved.

use crate::EditorSerial;
use ide_document::Revision;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use ide_core::settings::{Settings, keys};
use ide_lsp::Error;
use ide_lsp::lsp_types::{FormattingOptions, PrepareRenameResponse, TextEdit};
use ide_ui_common::alert::{Alert, Role, first, warn};
use objc2::MainThreadOnly;
use objc2_app_kit::NSTextField;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::{LspState, server_of, text};
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::util::{main_after, main_soon, ns};

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
pub fn rename(view: &dyn Editor) {
    let Some((server, uri)) = server_of(&editor::state(), view) else {
        return;
    };
    let index = view.selectedRange().location;
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.lsp().borrow().serial;
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
            ask_name(&*view, index, &current);
        });
    });
    if let Err(Error::Unsupported(_)) = started {
        let current = word_at(&view.string().to_string(), index);
        ask_name(view, index, &current);
    }
}

fn ask_name(view: &dyn Editor, index: usize, current: &str) {
    let Some(window) = view.window() else { return };
    let mtm = view.mtm();
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(280.0, 24.0)));
    let serial = view.lsp().borrow().serial;
    Alert::new(mtm, &tf("„{}“ umbenennen", &[current]))
        .informative(t("Der neue Name gilt in allen Dateien des Ordners."))
        .button(t("Umbenennen"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .accessory(&field)
        .focus(&field)
        .sheet(&window, move |response| {
            let name = field.stringValue().to_string();
            if first(response)
                && !name.trim().is_empty()
                && let Some(view) = editor::find_editor(serial)
            {
                rename_to(&*view, index, name.trim());
            }
        });
}

/// Renames the symbol at UTF-16 offset `index` to `name` in every file the server names.
#[cfg_attr(
    not(feature = "selftest"),
    expect(unreachable_pub, reason = "the self-test re-exports it")
)]
pub fn rename_to(view: &dyn Editor, index: usize, name: &str) {
    let Some((server, uri)) = server_of(&editor::state(), view) else {
        return;
    };
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let serial = view.lsp().borrow().serial;
    let _ = server.rename(&uri, position, name, move |answer| {
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            let Some(folder) = view.project_root() else {
                return;
            };
            let problem = match answer {
                Ok(Some(edit)) => {
                    return super::edits::apply_workspace_edit(&folder, &edit, |applied| {
                        if let (Err(failure), Some(mtm)) = (applied, objc2::MainThreadMarker::new()) {
                            warn(mtm, t("Umbenennen fehlgeschlagen"), &failure.message);
                        }
                    });
                }
                Ok(None) => t("Der Sprachserver nennt nichts umzubenennen.").to_string(),
                Err(error) => error.to_string(),
            };
            warn(view.mtm(), t("Umbenennen fehlgeschlagen"), &problem);
        });
    });
}

fn options(view: &dyn Editor) -> FormattingOptions {
    view.formatting_options()
}

/// What runs after formatting, such as saving.
pub(crate) type Then = Box<dyn FnOnce(&dyn Editor)>;

/// Formatting requests waiting for their answer, by the editor's serial.
struct Pending {
    id: u64,
    revision: Revision,
    then: Option<Then>,
}

/// The format-on-save requests that run, by editor.
#[derive(Default)]
pub(crate) struct Formatting {
    formatting: RefCell<HashMap<crate::EditorSerial, Pending>>,
    /// Prettier's runs by editor: the run's id and what waits for it.
    prettier: RefCell<HashMap<crate::EditorSerial, (u64, Option<Then>)>>,
}

impl Formatting {
    /// Keeps `then` until Prettier's run `id` for the editor `serial` ends.
    pub(crate) fn wait(&self, serial: EditorSerial, id: u64, then: Option<Then>) {
        self.prettier.borrow_mut().insert(serial, (id, then));
    }

    /// What waits for Prettier's run `id` for the editor `serial`; `None` once a later run took its place.
    pub(crate) fn take(&self, serial: EditorSerial, id: u64) -> Option<Option<Then>> {
        let mut runs = self.prettier.borrow_mut();
        if runs.get(&serial).is_some_and(|(run, _)| *run == id) {
            runs.remove(&serial).map(|(_, then)| then)
        } else {
            None
        }
    }

    /// Keeps `pending` for the editor `serial` until its answer comes; the request it takes the place of comes back.
    fn hold(&self, serial: EditorSerial, pending: Pending) -> Option<Pending> {
        self.formatting.borrow_mut().insert(serial, pending)
    }

    /// The request `id` of the editor `serial`, taken out; `None` once a later request took its place.
    fn finish(&self, serial: EditorSerial, id: u64) -> Option<Pending> {
        let mut all = self.formatting.borrow_mut();
        if all.get(&serial).is_some_and(|p| p.id == id) {
            all.remove(&serial)
        } else {
            None
        }
    }
}

/// Formats the document with its server, as one undoable step; `then`, such as saving, runs afterwards, also when
/// nothing changed or the server did not answer within `FORMAT_WAIT`.
pub fn format(view: &dyn Editor, then: Option<Then>) {
    let state = editor::state();
    let then = match super::prettier::format(&state, view, then) {
        Ok(()) => return,
        Err(then) => then,
    };
    let Some((server, uri)) = server_of(&state, view) else {
        if let Some(then) = then {
            then(view);
        }
        return;
    };
    let serial = view.lsp().borrow().serial;
    let id = super::next_id(&state);
    let started = server.format(&uri, options(view), move |answer| {
        main_soon(move || finished(serial, id, Some(answer)));
    });
    match started {
        Ok(_) => {
            let revision = view.document().revision();
            let waits = then.is_some();
            state.formatting.hold(serial, Pending { id, revision, then });
            if waits {
                main_after(FORMAT_WAIT, move || finished(serial, id, None));
            }
        }
        Err(error) => match then {
            Some(then) => then(view),
            None => warn(view.mtm(), t("Formatieren fehlgeschlagen"), &error.to_string()),
        },
    }
}

/// The answer to formatting request `id`, or `None` when the wait ran out first.
fn finished(serial: EditorSerial, id: u64, answer: Option<Result<Vec<TextEdit>, Error>>) {
    let pending = editor::state().formatting.finish(serial, id);
    let (Some(pending), Some(view)) = (pending, editor::find_editor(serial)) else {
        return;
    };
    match answer {
        Some(Ok(edits)) if view.document().revision() == pending.revision && !edits.is_empty() => {
            let ranges = super::edits::ranges(&*view, &edits);
            super::edits::apply_text_edits(&*view, ranges);
        }
        Some(Err(error)) => ide_ui_common::diag::report("formatting", error),
        _ => {}
    }
    if let Some(then) = pending.then {
        then(&*view);
    }
}

/// Saves `view` as ⌘S does, with `write`, such as `Editor::save_soon`: fixed and formatted first when the settings ask
/// for it and the servers can.
pub fn save(view: &dyn Editor, write: Then) {
    let wanted = view
        .project_root()
        .is_some_and(|root| Settings::load_user().in_project(&root).get(&keys::FORMAT_ON_SAVE));
    let state = editor::state();
    let can = server_of(&state, view).is_some_and(|(server, _)| {
        server
            .capabilities()
            .is_some_and(|c| c.document_formatting_provider.is_some())
    });
    let format_then_save = move |view: &dyn Editor| {
        if wanted && (can || super::prettier::formats(view)) {
            format(view, Some(write));
        } else {
            write(view);
        }
    };
    let fixes = ide_core::settings::Settings::load_user().get(&keys::ESLINT_FIX_ON_SAVE);
    match eslint_of(&state, view).filter(|_| fixes) {
        Some((server, uri)) => fix_all(&state, view, &server, &uri, Box::new(format_then_save)),
        None => format_then_save(view),
    }
}

/// Runs what waited for the fixes of request `id` of the editor `serial`, unless a later request took its place.
fn resume(serial: EditorSerial, id: u64) {
    let pending = editor::state().formatting.finish(serial, id);
    if let (Some(pending), Some(view)) = (pending, editor::find_editor(serial))
        && let Some(then) = pending.then
    {
        then(&*view);
    }
}

/// The running ESLint server of `view`, if one serves its file.
fn eslint_of(state: &LspState, view: &dyn Editor) -> Option<(ide_lsp::Server, ide_lsp::lsp_types::Uri)> {
    let root = view.project_root()?;
    let ids: Vec<u64> = view.lsp().borrow().companions.iter().map(|(id, _)| *id).collect();
    ids.into_iter()
        .find(|id| super::slot(state, &root, *id).is_some_and(|slot| slot.language == ide_lsp::Language::Eslint))
        .and_then(|id| super::server_by_id(state, view, id))
}

/// Runs ESLint's `source.fixAll.eslint` on the whole document, applied only to the revision it was asked for, then
/// `then`; also when it fails or takes longer than `FORMAT_WAIT` (E132).
fn fix_all(state: &LspState, view: &dyn Editor, server: &ide_lsp::Server, uri: &ide_lsp::lsp_types::Uri, then: Then) {
    use ide_lsp::lsp_types::{CodeActionKind, CodeActionOrCommand, Position, Range};
    let serial = view.lsp().borrow().serial;
    let revision = view.document().revision();
    let id = super::next_id(state);
    let lines = u32::try_from(view.document().text().line_count()).unwrap_or(u32::MAX);
    let whole = Range::new(Position::new(0, 0), Position::new(lines, 0));
    state.formatting.hold(
        serial,
        Pending {
            id,
            revision,
            then: Some(then),
        },
    );
    let only = Some(vec![CodeActionKind::new("source.fixAll.eslint")]);
    let started = server.code_actions(uri, whole, Vec::new(), only, move |answer| {
        main_soon(move || {
            let state = editor::state();
            let pending = state.formatting.finish(serial, id);
            let (Some(pending), Some(view)) = (pending, editor::find_editor(serial)) else {
                return;
            };
            let edit = answer.ok().and_then(|actions| {
                actions.into_iter().find_map(|action| match action {
                    CodeActionOrCommand::CodeAction(action) => action.edit,
                    CodeActionOrCommand::Command(_) => None,
                })
            });
            let folder = view.project_root();
            match (edit, folder) {
                (Some(edit), Some(folder)) if view.document().revision() == pending.revision => {
                    // What waits stays here on the main thread until the edit is applied.
                    state.formatting.hold(serial, pending);
                    super::edits::apply_workspace_edit(&folder, &edit, move |_| {
                        main_soon(move || resume(serial, id));
                    });
                }
                _ => {
                    if let Some(then) = pending.then {
                        then(&*view);
                    }
                }
            }
        });
    });
    if started.is_err() {
        let pending = state.formatting.formatting.borrow_mut().remove(&serial);
        if let Some(pending) = pending
            && let Some(then) = pending.then
        {
            then(view);
        }
        return;
    }
    main_after(FORMAT_WAIT, move || finished(serial, id, None));
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
