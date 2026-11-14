//! Replacing the matches of a search. An open document takes the replacement through its editor as one undo step per
//! file (E24); a closed file is read, changed and written through the document model as saving writes it, keeping its
//! encoding, byte order mark and line endings, on a thread of its own.

use std::io;
use std::path::{Path, PathBuf};

use ide_core::explain::Explain;
use ide_document::{Document, Edit, EditError, ReadOnly, SaveError};
use ide_project::{Replacement, SearchError, SearchOptions, replacements};
use objc2::rc::Retained;

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
    fn count(&mut self, path: &Path, result: Result<usize, Failure>) {
        match result {
            Ok(0) => {}
            Ok(n) => {
                self.replaced += n;
                self.files += 1;
            }
            Err(error) => self.problems.push(format!("{}: {}", path.display(), error.explain())),
        }
    }
}

/// Why a file kept its matches.
#[derive(Debug)]
enum Failure {
    /// Its editor does not let the user change it.
    ReadOnly,
    Search(SearchError),
    Open(io::Error),
    Edit(EditError),
    Save(SaveError),
}

impl Explain for Failure {
    fn explain(&self) -> String {
        match self {
            Failure::ReadOnly => ReadOnly::Protected.explain(),
            Failure::Search(error) => error.explain(),
            Failure::Open(error) => error.explain(),
            Failure::Edit(error) => error.explain(),
            Failure::Save(error) => error.explain(),
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
    let spawned = crate::util::spawn("ide-replace", move || {
        ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
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
fn in_editor(view: &EditorTextView, options: &SearchOptions, with: &str, only: &Only) -> Result<usize, Failure> {
    if !view.isEditable() {
        return Err(Failure::ReadOnly);
    }
    let text = view.document().text().to_string();
    let found = chosen(replacements(&text, options, with).map_err(Failure::Search)?, only);
    let count = found.len();
    if count > 0 {
        crate::lsp::apply_text_edits(view, found.into_iter().map(|r| (r.range_utf16, r.text)).collect());
    }
    Ok(count)
}

/// Replaces in a file no editor shows and saves it.
fn in_file(path: &Path, options: &SearchOptions, with: &str, only: &Only) -> Result<usize, Failure> {
    let mut document = Document::open(path).map_err(Failure::Open)?;
    let text = document.text().to_string();
    let found = chosen(replacements(&text, options, with).map_err(Failure::Search)?, only);
    if found.is_empty() {
        return Ok(0);
    }
    for replacement in found.iter().rev() {
        document
            .apply(Edit {
                range_utf16: replacement.range_utf16.clone(),
                text: replacement.text.clone(),
            })
            .map_err(Failure::Edit)?;
    }
    document.save().map_err(Failure::Save)?;
    Ok(found.len())
}

#[cfg(test)]
mod tests {
    use ide_core::testing::Scratch;

    use super::*;

    fn file(scratch: &Scratch, text: &str) -> PathBuf {
        let path = scratch.path().join("a.txt");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_closed_file_gets_every_match_replaced_and_saved() {
        let scratch = Scratch::new("replace-all");
        let path = file(&scratch, "cat\r\nCat cat\r\n");
        let count = in_file(&path, &SearchOptions::new("cat"), "dog", &Only::All).unwrap();
        assert_eq!(count, 3);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "dog\r\ndog dog\r\n");
    }

    #[test]
    fn replacing_one_line_leaves_the_other_matches() {
        let scratch = Scratch::new("replace-line");
        let path = file(&scratch, "cat\ncat cat\n");
        let match_columns = 4..7;
        let only = Only::Line(2, vec![match_columns]);
        assert_eq!(in_file(&path, &SearchOptions::new("cat"), "dog", &only).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "cat\ncat dog\n");
    }

    #[test]
    fn a_file_without_matches_is_not_written() {
        let scratch = Scratch::new("replace-none");
        let path = file(&scratch, "nothing\n");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            in_file(&path, &SearchOptions::new("cat"), "dog", &Only::All).unwrap(),
            0
        );
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn the_tally_counts_files_that_changed_and_names_those_that_failed() {
        let mut done = Done::default();
        done.count(Path::new("/a"), Ok(2));
        done.count(Path::new("/b"), Ok(0));
        done.count(Path::new("/c"), Err(Failure::ReadOnly));
        assert_eq!((done.replaced, done.files), (2, 1));
        assert_eq!(done.problems, [format!("/c: {}", Failure::ReadOnly.explain())]);
    }

    #[test]
    fn a_missing_file_is_a_problem_not_a_panic() {
        let scratch = Scratch::new("replace-missing");
        let path = scratch.path().join("gone.txt");
        assert!(in_file(&path, &SearchOptions::new("cat"), "dog", &Only::All).is_err());
    }
}
