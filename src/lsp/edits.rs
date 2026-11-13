//! Edits a server asks for, in open editors as one undoable step each, through `apply` for a whole workspace; and the
//! questions a server may ask the user.

use ide_core::LockIgnoringPoison;
use std::cell::RefCell;
use std::ops::Range;
use std::path::Path;

use block2::RcBlock;
use ide_lsp::Reply;
use ide_lsp::lsp_types::{MessageActionItem, TextEdit, WorkspaceEdit};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSModalResponse};
use objc2_foundation::{NSFileManager, NSRange, NSURL};
use splat_i18n::{t, tf};

use super::apply::Failure;
use crate::editor::{self, EditorTextView};
use crate::util::{guarded, ns};

/// Replaces `edits`, UTF-16 ranges of the text as it is now that do not overlap, as one undoable step that flows into
/// the document model like typing; whether the view took every one.
pub fn apply_text_edits(view: &EditorTextView, edits: Vec<(Range<usize>, String)>) -> bool {
    let Some(storage) = view.storage() else { return false };
    let mut took = true;
    editor::undo_step(view, || {
        for (range, text) in super::apply::back_to_front(edits) {
            let range = NSRange::new(range.start, range.end - range.start);
            let text = ns(&text);
            if view.shouldChangeTextInRange_replacementString(range, Some(&text)) {
                storage.replaceCharactersInRange_withString(range, &text);
                view.didChangeText();
            } else {
                took = false;
            }
        }
    });
    took
}

/// `edits` of the protocol as ranges of `view`'s current text.
pub fn ranges(view: &EditorTextView, edits: &[TextEdit]) -> Vec<(Range<usize>, String)> {
    let document = view.document();
    edits
        .iter()
        .map(|e| (super::text::range(document.text(), e.range), e.new_text.clone()))
        .collect()
}

/// Applies `edit` to the files under `folder` and to every open editor (Review C1), on the disk lane, and tells `done`
/// on the main thread.
pub fn apply_workspace_edit(
    folder: &Path,
    edit: &WorkspaceEdit,
    done: impl FnOnce(Result<(), Failure>) + Send + 'static,
) {
    let not_run = || Failure {
        message: t("Die Änderung des Arbeitsbereichs ließ sich nicht ausführen.").to_string(),
        index: None,
    };
    let (folder, edit) = (folder.to_path_buf(), edit.clone());
    let done = std::sync::Arc::new(std::sync::Mutex::new(Some(done)));
    let answer = done.clone();
    let queued = ide_core::lane::DISK.ask(
        move || super::apply::apply(&folder, &edit, &mut FromLane),
        move |result| {
            let result = result.unwrap_or_else(|| Err(not_run()));
            crate::util::main_soon(move || {
                if let Some(done) = answer.lock_ignoring_poison().take() {
                    done(result);
                }
            });
        },
    );
    if queued.is_err()
        && let Some(done) = done.lock_ignoring_poison().take()
    {
        done(Err(not_run()));
    }
}

/// Whether `shown`, the path of an editor's file, names `path`, whose symlinks are resolved; also for a file that
/// was just renamed away, whose folder still resolves.
fn names(shown: &Path, path: &Path) -> bool {
    let resolved = shown.canonicalize().ok().or_else(|| {
        let folder = shown.parent()?.canonicalize().ok()?;
        Some(folder.join(shown.file_name()?))
    });
    shown == path || resolved.is_some_and(|resolved| resolved == path)
}

/// The editor that shows the file at `path`, whose symlinks are resolved.
fn editor_at(path: &Path) -> Option<Retained<EditorTextView>> {
    editor::all_editors()
        .into_iter()
        .find(|view| view.document().path().is_some_and(|shown| names(shown, path)))
}

/// The app's editors, as a workspace edit reaches them.
struct OpenEditors;

/// The app's editors as the disk lane reaches them: each question waits for its answer from the main thread.
struct FromLane;

/// What `f` answers on the main thread, for a worker thread that waits for it.
fn on_main<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    let mut answer = None;
    dispatch2::DispatchQueue::main().exec_sync(|| answer = guarded(None, || Some(f())));
    answer.expect("the main thread answers unless the code there panicked")
}

impl super::apply::Editors for FromLane {
    fn version(&self, path: &Path) -> Option<i32> {
        on_main(|| OpenEditors.version(path))
    }

    fn edit(&mut self, path: &Path, edits: &[TextEdit]) -> Option<Result<(), String>> {
        on_main(|| OpenEditors.edit(path, edits))
    }

    fn moved(&mut self, from: &Path, to: &Path) {
        on_main(|| OpenEditors.moved(from, to));
    }

    fn trash(&mut self, path: &Path) -> Result<(), String> {
        OpenEditors.trash(path)
    }
}

impl super::apply::Editors for OpenEditors {
    fn version(&self, path: &Path) -> Option<i32> {
        editor_at(path).map(|view| super::version(&view))
    }

    fn edit(&mut self, path: &Path, edits: &[TextEdit]) -> Option<Result<(), String>> {
        let view = editor_at(path)?;
        let refused = || tf("{} lässt sich nicht ändern", &[&path.display().to_string()]);
        if view.document().read_only().is_some() {
            return Some(Err(refused()));
        }
        let ranges = ranges(&view, edits);
        Some(if apply_text_edits(&view, ranges) {
            Ok(())
        } else {
            Err(refused())
        })
    }

    fn moved(&mut self, from: &Path, to: &Path) {
        for view in editor::all_editors() {
            if view.document().path().is_some_and(|shown| names(shown, from)) {
                view.document_mut().set_path(to.to_path_buf());
                editor::path_changed(&view);
            }
        }
    }

    fn trash(&mut self, path: &Path) -> Result<(), String> {
        let url = NSURL::fileURLWithPath(&ns(&path.to_string_lossy()));
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|e| format!("{}: {}", path.display(), e.localizedDescription()))
    }
}

/// Shows a server's question with its answers as a sheet of the window of `folder`; closing it answers none.
pub fn ask(folder: &Path, message: &str, actions: Vec<MessageActionItem>, reply: Reply<Option<MessageActionItem>>) {
    question(folder, message, actions, move |chosen| reply.send(chosen));
}

/// Shows `message` with up to three of `actions` as buttons in a sheet of the window of `folder`; `answered` hears the
/// one chosen, or `None` when the sheet closed otherwise.
pub fn question(
    folder: &Path,
    message: &str,
    actions: Vec<MessageActionItem>,
    answered: impl FnOnce(Option<MessageActionItem>) + 'static,
) {
    let Some(window) = crate::window::for_project(folder).and_then(|c| c.window()) else {
        return;
    };
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(message));
    let shown = actions.len().min(3);
    for action in &actions[..shown] {
        alert.addButtonWithTitle(&ns(&action.title));
    }
    alert.addButtonWithTitle(&ns(t("Schließen")));
    let answered = RefCell::new(Some(answered));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            let chosen = usize::try_from(response - NSAlertFirstButtonReturn)
                .ok()
                .filter(|i| *i < shown)
                .and_then(|i| actions.get(i).cloned());
            if let Some(answered) = answered.borrow_mut().take() {
                answered(chosen);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
}
