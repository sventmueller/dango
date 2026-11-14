use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use lsp_types::{
    CreateFile, DocumentChangeOperation, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Position,
    Range, RenameFile, ResourceOp, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

use super::*;

/// An editor that shows a file: its text, the version the server knows, and the texts before each undo step.
struct Shown {
    text: String,
    version: i32,
    undo: Vec<String>,
}

#[derive(Default)]
struct Mock {
    shown: HashMap<PathBuf, Shown>,
}

impl Mock {
    fn show(&mut self, path: &Path, text: &str, version: i32) {
        let shown = Shown {
            text: text.into(),
            version,
            undo: vec![],
        };
        self.shown.insert(path.to_path_buf(), shown);
    }

    fn text(&self, path: &Path) -> &str {
        &self.shown[path].text
    }

    fn undo(&mut self, path: &Path) {
        let shown = self.shown.get_mut(path).expect("shown");
        shown.text = shown.undo.pop().expect("an undo step");
    }
}

impl Editors for Mock {
    fn version(&self, path: &Path) -> Option<i32> {
        self.shown.get(path).map(|s| s.version)
    }

    fn edit(&mut self, path: &Path, edits: &[TextEdit]) -> Option<Result<(), StepError>> {
        let shown = self.shown.get_mut(path)?;
        let mut document = Document::untitled(&shown.text);
        let ranges = edits
            .iter()
            .map(|e| (crate::offsets::range(document.text(), e.range), e.new_text.clone()))
            .collect();
        for (range, text) in back_to_front(ranges) {
            document
                .apply(Edit {
                    range_utf16: range,
                    text,
                })
                .ok()?;
        }
        let before = std::mem::replace(&mut shown.text, document.text().to_string());
        shown.undo.push(before);
        shown.version += 1;
        Some(Ok(()))
    }

    fn moved(&mut self, from: &Path, to: &Path) {
        if let Some(shown) = self.shown.remove(from) {
            self.shown.insert(to.to_path_buf(), shown);
        }
    }

    fn trash(&mut self, path: &Path) -> Result<(), StepError> {
        std::fs::remove_file(path).map_err(|error| StepError::Io {
            path: path.to_path_buf(),
            error,
        })
    }
}

/// A fresh folder for one test, with its symlinks resolved as the app's folders are.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-apply-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn uri(path: &Path) -> lsp_types::Uri {
    crate::uri::from_path(path)
}

/// Replaces line 0, columns `from..to`, with `text`.
fn replace(from: u32, to: u32, text: &str) -> TextEdit {
    TextEdit::new(Range::new(Position::new(0, from), Position::new(0, to)), text.into())
}

fn change(path: &Path, version: Option<i32>, edits: Vec<TextEdit>) -> DocumentChangeOperation {
    DocumentChangeOperation::Edit(TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier {
            uri: uri(path),
            version,
        },
        edits: edits.into_iter().map(OneOf::Left).collect(),
    })
}

fn rename(from: &Path, to: &Path) -> DocumentChangeOperation {
    DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
        old_uri: uri(from),
        new_uri: uri(to),
        options: None,
        annotation_id: None,
    }))
}

fn create(path: &Path) -> DocumentChangeOperation {
    DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
        uri: uri(path),
        options: None,
        annotation_id: None,
    }))
}

fn operations(all: Vec<DocumentChangeOperation>) -> WorkspaceEdit {
    WorkspaceEdit {
        document_changes: Some(DocumentChanges::Operations(all)),
        ..WorkspaceEdit::default()
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| format!("<{e}>"))
}

#[test]
fn an_edit_then_a_rename_edits_the_file_that_is_then_renamed() {
    let dir = scratch("edit-rename");
    let (closed, moved) = (dir.join("closed.rs"), dir.join("moved.rs"));
    let (open, renamed) = (dir.join("open.rs"), dir.join("renamed.rs"));
    std::fs::write(&closed, "old name\n").unwrap();
    std::fs::write(&open, "old text\n").unwrap();
    let mut editors = Mock::default();
    editors.show(&open, "old text\n", 7);
    let edit = operations(vec![
        change(&closed, None, vec![replace(0, 3, "new")]),
        rename(&closed, &moved),
        change(&open, Some(7), vec![replace(0, 3, "new")]),
        rename(&open, &renamed),
    ]);
    assert_eq!(apply(&dir, &edit, &mut editors), Ok(()));
    assert_eq!(read(&moved), "new name\n", "the closed file was edited before it moved");
    assert!(!closed.exists());
    assert_eq!(editors.text(&renamed), "new text\n", "the editor followed its file");
    assert!(!open.exists() && renamed.exists());
}

#[test]
fn a_rename_then_an_edit_finds_the_editor_under_its_new_name() {
    let dir = scratch("rename-edit");
    let (open, renamed) = (dir.join("open.rs"), dir.join("renamed.rs"));
    std::fs::write(&open, "text\n").unwrap();
    let mut editors = Mock::default();
    editors.show(&open, "text\n", 2);
    let edit = operations(vec![
        rename(&open, &renamed),
        change(&renamed, Some(2), vec![replace(0, 0, "more ")]),
    ]);
    assert_eq!(apply(&dir, &edit, &mut editors), Ok(()));
    assert_eq!(editors.text(&renamed), "more text\n");
}

#[test]
fn a_stale_version_drops_the_whole_edit_before_anything_changes() {
    let dir = scratch("stale");
    let (closed, open) = (dir.join("closed.rs"), dir.join("open.rs"));
    std::fs::write(&closed, "closed\n").unwrap();
    std::fs::write(&open, "open\n").unwrap();
    let mut editors = Mock::default();
    editors.show(&open, "typed since\n", 5);
    let edit = operations(vec![
        change(&closed, None, vec![replace(0, 0, "x")]),
        create(&dir.join("new.rs")),
        change(&open, Some(4), vec![replace(0, 0, "x")]),
    ]);
    let failure = apply(&dir, &edit, &mut editors).unwrap_err();
    assert!(
        failure.message.starts_with("Die Datei hat sich inzwischen geändert"),
        "{failure:?}"
    );
    assert_eq!(failure.index, Some(2));
    assert_eq!(read(&closed), "closed\n");
    assert!(!dir.join("new.rs").exists());
    assert_eq!(editors.text(&open), "typed since\n");
    let closed_since = operations(vec![change(&dir.join("closed.rs"), Some(1), vec![replace(0, 0, "x")])]);
    assert!(
        apply(&dir, &closed_since, &mut editors).is_err(),
        "a version for a document no editor shows any longer is stale too"
    );
}

#[test]
fn a_symlink_out_of_the_folder_drops_the_whole_edit() {
    let dir = scratch("symlink");
    let outside = scratch("symlink-outside");
    std::fs::write(outside.join("target.rs"), "outside\n").unwrap();
    std::fs::write(dir.join("inside.rs"), "inside\n").unwrap();
    std::os::unix::fs::symlink(outside.join("target.rs"), dir.join("link.rs")).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("away")).unwrap();
    let mut editors = Mock::default();
    let first = change(&dir.join("inside.rs"), None, vec![replace(0, 0, "x")]);
    for last in [
        change(&dir.join("link.rs"), None, vec![replace(0, 0, "x")]),
        create(&dir.join("away/new.rs")),
        rename(&dir.join("inside.rs"), &dir.join("away/inside.rs")),
        create(&dir.join("not-there/../../escaped.rs")),
    ] {
        let failure = apply(&dir, &operations(vec![first.clone(), last.clone()]), &mut editors).unwrap_err();
        assert!(
            failure.message.contains("außerhalb des Ordners"),
            "{last:?}: {failure:?}"
        );
        assert_eq!(failure.index, Some(1));
        assert_eq!(
            read(&dir.join("inside.rs")),
            "inside\n",
            "nothing ran before the check failed"
        );
    }
    assert_eq!(read(&outside.join("target.rs")), "outside\n");
    assert!(!outside.join("new.rs").exists() && !outside.join("inside.rs").exists());
}

#[test]
fn after_a_failure_midway_the_message_names_what_changed_and_undo_takes_the_open_file_back() {
    let dir = scratch("partial");
    let (open, written, locked) = (dir.join("open.rs"), dir.join("written.rs"), dir.join("locked.rs"));
    std::fs::write(&open, "open\n").unwrap();
    std::fs::write(&written, "written\n").unwrap();
    std::fs::write(&locked, "locked\n").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    let mut editors = Mock::default();
    editors.show(&open, "open\n", 1);
    let edit = operations(vec![
        change(&open, Some(1), vec![replace(0, 4, "changed")]),
        change(&written, None, vec![replace(0, 7, "changed")]),
        change(&locked, None, vec![replace(0, 6, "changed")]),
        create(&dir.join("never.rs")),
    ]);
    let failure = apply(&dir, &edit, &mut editors).unwrap_err();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(failure.index, Some(2));
    assert!(failure.message.contains("locked.rs"), "{failure:?}");
    assert!(
        failure.message.contains("Schon geändert: open.rs, written.rs"),
        "{failure:?}"
    );
    assert!(failure.message.contains("widerrufen"), "{failure:?}");
    assert_eq!(read(&written), "changed\n");
    assert_eq!(read(&locked), "locked\n");
    assert!(!dir.join("never.rs").exists(), "nothing runs after the failure");
    assert_eq!(editors.text(&open), "changed\n");
    editors.undo(&open);
    assert_eq!(editors.text(&open), "open\n", "one undo step takes the open file back");
}

#[test]
fn a_closed_file_keeps_its_line_breaks_and_inserts_at_one_place_keep_their_order() {
    let dir = scratch("closed");
    let file = dir.join("crlf.rs");
    std::fs::write(&file, "ab\r\ncd\r\n").unwrap();
    let edits = vec![replace(1, 1, "1"), replace(1, 1, "2"), replace(0, 0, "<")];
    let edit = WorkspaceEdit {
        changes: Some(std::iter::once((uri(&file), edits)).collect()),
        ..WorkspaceEdit::default()
    };
    assert_eq!(apply(&dir, &edit, &mut Mock::default()), Ok(()));
    assert_eq!(read(&file), "<a12b\r\ncd\r\n");
}

#[test]
fn a_create_that_exists_fails_unless_asked_to_ignore_or_overwrite() {
    let dir = scratch("create");
    let file = dir.join("there.rs");
    std::fs::write(&file, "kept\n").unwrap();
    let failure = apply(&dir, &operations(vec![create(&file)]), &mut Mock::default()).unwrap_err();
    assert!(failure.message.contains("gibt es schon"), "{failure:?}");
    let ignoring = DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
        uri: uri(&file),
        options: Some(lsp_types::CreateFileOptions {
            overwrite: None,
            ignore_if_exists: Some(true),
        }),
        annotation_id: None,
    }));
    assert_eq!(apply(&dir, &operations(vec![ignoring]), &mut Mock::default()), Ok(()));
    assert_eq!(read(&file), "kept\n");
}
