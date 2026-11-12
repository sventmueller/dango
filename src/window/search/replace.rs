//! Replacing the matches of a search. An open document takes the replacement through its editor as one undo step per
//! file (E24); a closed file is read, changed and written through the document model as saving writes it, keeping its
//! encoding, byte order mark and line endings, on a thread of its own.

use std::path::{Path, PathBuf};

use ide_document::{Document, Edit};
use ide_project::{Replacement, SearchOptions, replacements};
use objc2::rc::Retained;
use splat_i18n::t;

use super::model::Only;
use crate::editor::EditorTextView;
use crate::util::main_soon;

/// What a replacement did.
#[derive(Debug, Default)]
pub(super) struct Done {
    pub(super) replaced: usize,
    pub(super) files: usize,
    pub(super) problems: Vec<String>,
}

impl Done {
    fn count(&mut self, path: &Path, result: Result<usize, String>) {
        match result {
            Ok(0) => {}
            Ok(n) => {
                self.replaced += n;
                self.files += 1;
            }
            Err(error) => self.problems.push(format!("{}: {error}", path.display())),
        }
    }
}

/// Replaces in `files` the matches of `options` with `with`; `done` runs on the main thread once every file is done.
pub(super) fn replace(
    files: Vec<PathBuf>,
    options: SearchOptions,
    with: String,
    only: Only,
    done: impl FnOnce(Done) + Send + 'static,
) {
    let mut result = Done::default();
    let mut closed = Vec::new();
    for path in files {
        match open_editor(&path) {
            Some(view) => result.count(&path, in_editor(&view, &options, &with, &only)),
            None => closed.push(path),
        }
    }
    if closed.is_empty() {
        done(result);
        return;
    }
    let spawned = std::thread::Builder::new().name("ide-replace".into()).spawn(move || {
        for path in &closed {
            result.count(path, in_file(path, &options, &with, &only));
        }
        main_soon(move || done(result));
    });
    if let Err(error) = spawned {
        crate::diag::report("replacing in closed files", error);
    }
}

/// The editor that shows the file at `path`, whose document all its editors share.
fn open_editor(path: &Path) -> Option<Retained<EditorTextView>> {
    let wanted = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    crate::editor::all_editors().into_iter().find(|view| {
        view.document()
            .path()
            .is_some_and(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()) == wanted)
    })
}

fn chosen(found: Vec<Replacement>, only: &Only) -> Vec<Replacement> {
    match only {
        Only::All => found,
        Only::Line(line, columns) => found
            .into_iter()
            .filter(|r| r.line == *line && columns.contains(&r.columns))
            .collect(),
    }
}

/// Replaces in the text of an open document, as one undo step of its editor.
fn in_editor(view: &EditorTextView, options: &SearchOptions, with: &str, only: &Only) -> Result<usize, String> {
    if !view.isEditable() {
        return Err(t("Die Datei ist schreibgeschützt.").to_string());
    }
    let text = view.document().text().to_string();
    let found = chosen(replacements(&text, options, with).map_err(|e| e.to_string())?, only);
    let count = found.len();
    if count > 0 {
        crate::lsp::apply_text_edits(view, found.into_iter().map(|r| (r.range_utf16, r.text)).collect());
    }
    Ok(count)
}

/// Replaces in a file no editor shows and saves it.
fn in_file(path: &Path, options: &SearchOptions, with: &str, only: &Only) -> Result<usize, String> {
    let mut document = Document::open(path).map_err(|e| e.to_string())?;
    let text = document.text().to_string();
    let found = chosen(replacements(&text, options, with).map_err(|e| e.to_string())?, only);
    if found.is_empty() {
        return Ok(0);
    }
    for replacement in found.iter().rev() {
        document
            .apply(Edit {
                range_utf16: replacement.range_utf16.clone(),
                text: replacement.text.clone(),
            })
            .map_err(|e| format!("{e:?}"))?;
    }
    document.save().map_err(|e| format!("{e:?}"))?;
    Ok(found.len())
}
