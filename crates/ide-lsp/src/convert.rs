//! Every position inside a message, with the document it belongs to, so it can be recounted between the editor's
//! UTF-16 columns and the server's unit.

use lsp_types::{
    CodeAction, CodeActionOrCommand, CompletionItem, CompletionTextEdit, Diagnostic, DocumentChangeOperation,
    DocumentChanges, DocumentSymbol, InlayHint, Location, LocationLink, OneOf, Position, PrepareRenameResponse, Range,
    SymbolInformation, TextEdit, Uri, WorkspaceEdit,
};

/// A message part that holds positions; `uri` is the document positions without their own URI belong to.
pub(crate) trait Positions {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position));
}

impl Positions for Position {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        f(uri, self);
    }
}

impl Positions for Range {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        f(uri, &mut self.start);
        f(uri, &mut self.end);
    }
}

impl<T: Positions> Positions for Vec<T> {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        for item in self {
            item.each(uri, f);
        }
    }
}

impl<T: Positions> Positions for Option<T> {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        if let Some(item) = self {
            item.each(uri, f);
        }
    }
}

impl Positions for TextEdit {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.range.each(uri, f);
    }
}

impl Positions for Location {
    fn each(&mut self, _: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.range.each(&self.uri, f);
    }
}

impl Positions for LocationLink {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.origin_selection_range.each(uri, f);
        self.target_range.each(&self.target_uri, f);
        self.target_selection_range.each(&self.target_uri, f);
    }
}

impl Positions for Diagnostic {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.range.each(uri, f);
        for related in self.related_information.iter_mut().flatten() {
            related.location.each(uri, f);
        }
    }
}

impl Positions for WorkspaceEdit {
    fn each(&mut self, _: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        for (uri, edits) in self.changes.iter_mut().flatten() {
            edits.each(uri, f);
        }
        let edits: Vec<&mut lsp_types::TextDocumentEdit> = match &mut self.document_changes {
            None => vec![],
            Some(DocumentChanges::Edits(edits)) => edits.iter_mut().collect(),
            Some(DocumentChanges::Operations(operations)) => operations
                .iter_mut()
                .filter_map(|op| match op {
                    DocumentChangeOperation::Edit(edit) => Some(edit),
                    DocumentChangeOperation::Op(_) => None,
                })
                .collect(),
        };
        for edit in edits {
            let uri = &edit.text_document.uri;
            for one in &mut edit.edits {
                match one {
                    OneOf::Left(plain) => plain.each(uri, f),
                    OneOf::Right(annotated) => annotated.text_edit.each(uri, f),
                }
            }
        }
    }
}

impl Positions for CompletionItem {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        match &mut self.text_edit {
            Some(CompletionTextEdit::Edit(edit)) => edit.each(uri, f),
            Some(CompletionTextEdit::InsertAndReplace(edit)) => {
                edit.insert.each(uri, f);
                edit.replace.each(uri, f);
            }
            None => {}
        }
        self.additional_text_edits.each(uri, f);
    }
}

impl Positions for DocumentSymbol {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.range.each(uri, f);
        self.selection_range.each(uri, f);
        self.children.each(uri, f);
    }
}

impl Positions for SymbolInformation {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.location.each(uri, f);
    }
}

impl Positions for CodeAction {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.diagnostics.each(uri, f);
        self.edit.each(uri, f);
    }
}

impl Positions for CodeActionOrCommand {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        if let CodeActionOrCommand::CodeAction(action) = self {
            action.each(uri, f);
        }
    }
}

impl Positions for InlayHint {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        self.position.each(uri, f);
        self.text_edits.each(uri, f);
    }
}

impl Positions for PrepareRenameResponse {
    fn each(&mut self, uri: &Uri, f: &mut dyn FnMut(&Uri, &mut Position)) {
        match self {
            PrepareRenameResponse::Range(range) | PrepareRenameResponse::RangeWithPlaceholder { range, .. } => {
                range.each(uri, f);
            }
            PrepareRenameResponse::DefaultBehavior { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;

    use lsp_types::{OptionalVersionedTextDocumentIdentifier, TextDocumentEdit};

    use super::*;

    fn uri(s: &str) -> Uri {
        Uri::from_str(s).unwrap()
    }

    #[test]
    fn every_position_of_a_workspace_edit_is_visited_with_its_own_document() {
        let edit = |line| TextEdit::new(Range::new(Position::new(line, 1), Position::new(line, 2)), "x".into());
        let mut workspace = WorkspaceEdit {
            changes: Some(HashMap::from([(uri("file:///a"), vec![edit(1)])])),
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    uri: uri("file:///b"),
                    version: None,
                },
                edits: vec![OneOf::Left(edit(2))],
            }])),
            change_annotations: None,
        };
        let mut seen = vec![];
        workspace.each(&uri("file:///unused"), &mut |u, p| {
            seen.push((u.as_str().to_string(), p.line));
            p.character += 10;
        });
        seen.sort();
        assert_eq!(
            seen,
            vec![
                ("file:///a".into(), 1),
                ("file:///a".into(), 1),
                ("file:///b".into(), 2),
                ("file:///b".into(), 2)
            ]
        );
        assert_eq!(
            workspace.changes.unwrap()[&uri("file:///a")][0].range.start.character,
            11
        );
    }

    #[test]
    fn a_link_keeps_its_origin_in_the_asking_document() {
        let r = Range::new(Position::new(0, 0), Position::new(0, 1));
        let mut link = LocationLink {
            origin_selection_range: Some(r),
            target_uri: uri("file:///target"),
            target_range: r,
            target_selection_range: r,
        };
        let mut seen = vec![];
        link.each(&uri("file:///origin"), &mut |u, _| seen.push(u.as_str().to_string()));
        assert_eq!(&seen[..2], ["file:///origin", "file:///origin"]);
        assert!(seen[2..].iter().all(|u| u == "file:///target"));
    }
}
