//! Edits a server asks for: in an open editor as one undoable step, in a file nobody has open through the document
//! model, so its encoding and line breaks stay; and the questions a server may ask the user.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use block2::RcBlock;
use ide_document::{Document, Edit};
use ide_lsp::Reply;
use ide_lsp::lsp_types::{
    DocumentChangeOperation, DocumentChanges, MessageActionItem, OneOf, ResourceOp, TextEdit, WorkspaceEdit,
};
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSModalResponse};
use objc2_foundation::{NSFileManager, NSRange, NSURL};
use splat_i18n::{t, tf};

use crate::editor::{self, EditorTextView};
use crate::util::{guarded, ns};

/// Replaces `edits`, UTF-16 ranges of the text as it is now that do not overlap, as one undoable step that flows into
/// the document model like typing.
pub fn apply_text_edits(view: &EditorTextView, mut edits: Vec<(Range<usize>, String)>) {
    let Some(storage) = view.storage() else { return };
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    editor::undo_step(view, || {
        for (range, text) in edits {
            let range = NSRange::new(range.start, range.end - range.start);
            let text = ns(&text);
            if view.shouldChangeTextInRange_replacementString(range, Some(&text)) {
                storage.replaceCharactersInRange_withString(range, &text);
                view.didChangeText();
            }
        }
    });
}

/// `edits` of the protocol as ranges of `view`'s current text.
pub fn ranges(view: &EditorTextView, edits: &[TextEdit]) -> Vec<(Range<usize>, String)> {
    let document = view.document();
    edits
        .iter()
        .map(|e| (super::text::range(document.text(), e.range), e.new_text.clone()))
        .collect()
}

/// The text edits of `edit` by file, and its file operations, in order.
fn split(edit: &WorkspaceEdit) -> (BTreeMap<PathBuf, Vec<TextEdit>>, Vec<ResourceOp>) {
    let mut files: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
    let mut operations = vec![];
    for (uri, edits) in edit.changes.iter().flatten() {
        if let Some(path) = ide_lsp::uri::to_path(uri) {
            files.entry(path).or_default().extend(edits.iter().cloned());
        }
    }
    let mut add = |document: &ide_lsp::lsp_types::TextDocumentEdit| {
        if let Some(path) = ide_lsp::uri::to_path(&document.text_document.uri) {
            let edits = document.edits.iter().map(|one| match one {
                OneOf::Left(edit) => edit.clone(),
                OneOf::Right(annotated) => annotated.text_edit.clone(),
            });
            files.entry(path).or_default().extend(edits);
        }
    };
    match &edit.document_changes {
        Some(DocumentChanges::Edits(documents)) => documents.iter().for_each(&mut add),
        Some(DocumentChanges::Operations(all)) => {
            for operation in all {
                match operation {
                    DocumentChangeOperation::Edit(document) => add(document),
                    DocumentChangeOperation::Op(op) => operations.push(op.clone()),
                }
            }
        }
        None => {}
    }
    (files, operations)
}

/// Applies `edit` to the files under `folder` and to every open editor; the error says what failed.
pub fn apply_workspace_edit(folder: &Path, edit: &WorkspaceEdit) -> Result<(), String> {
    let (files, operations) = split(edit);
    for operation in &operations {
        resource(operation)?;
    }
    let mut problems = vec![];
    for (path, edits) in files {
        if !path.starts_with(folder) {
            problems.push(tf("{} liegt außerhalb des Ordners", &[&path.display().to_string()]));
            continue;
        }
        let open = editor::all_editors()
            .into_iter()
            .find(|v| v.document().path() == Some(&path));
        match open {
            Some(view) => {
                let ranges = ranges(&view, &edits);
                apply_text_edits(&view, ranges);
            }
            None => {
                if let Err(error) = edit_file(&path, &edits) {
                    problems.push(format!("{}: {error}", path.display()));
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Edits a file that no editor shows, through the document model, and saves it.
fn edit_file(path: &Path, edits: &[TextEdit]) -> Result<(), String> {
    let mut document = Document::open(path).map_err(|e| e.to_string())?;
    let mut ranges: Vec<(Range<usize>, String)> = edits
        .iter()
        .map(|e| (super::text::range(document.text(), e.range), e.new_text.clone()))
        .collect();
    ranges.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, text) in ranges {
        document
            .apply(Edit {
                range_utf16: range,
                text,
            })
            .map_err(|e| format!("{e:?}"))?;
    }
    document.save().map_err(|e| format!("{e:?}"))
}

/// Creates, renames or deletes a file as a server's edit asks; a deleted file goes to the Trash.
fn resource(operation: &ResourceOp) -> Result<(), String> {
    let path = |uri| ide_lsp::uri::to_path(uri).ok_or_else(|| t("Keine Datei").to_string());
    match operation {
        ResourceOp::Create(create) => {
            let file = path(&create.uri)?;
            let options = create.options.as_ref();
            if file.exists() && !options.and_then(|o| o.overwrite).unwrap_or(false) {
                return Ok(());
            }
            std::fs::write(&file, "").map_err(|e| e.to_string())
        }
        ResourceOp::Rename(rename) => {
            let (from, to) = (path(&rename.old_uri)?, path(&rename.new_uri)?);
            if to.exists() && !rename.options.as_ref().and_then(|o| o.overwrite).unwrap_or(false) {
                return Err(tf("{} gibt es schon", &[&to.display().to_string()]));
            }
            std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
            for view in editor::all_editors() {
                if view.document().path() == Some(&from) {
                    view.document_mut().set_path(to.clone());
                    editor::path_changed(&view);
                }
            }
            Ok(())
        }
        ResourceOp::Delete(delete) => {
            let file = path(&delete.uri)?;
            let url = NSURL::fileURLWithPath(&ns(&file.to_string_lossy()));
            NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, None)
                .map_err(|e| e.localizedDescription().to_string())
        }
    }
}

/// Shows a server's question with its answers as a sheet of the window of `folder`; closing it answers none.
pub fn ask(folder: &Path, message: &str, actions: Vec<MessageActionItem>, reply: Reply<Option<MessageActionItem>>) {
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
    let reply = RefCell::new(Some(reply));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            let chosen = usize::try_from(response - NSAlertFirstButtonReturn)
                .ok()
                .filter(|i| *i < shown)
                .and_then(|i| actions.get(i).cloned());
            if let Some(reply) = reply.borrow_mut().take() {
                reply.send(chosen);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
}
