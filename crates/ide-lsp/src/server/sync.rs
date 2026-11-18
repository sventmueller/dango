//! Keeping the server in step with the open documents, as far as its capabilities ask for.

use lsp_types::{TextDocumentContentChangeEvent, Uri};
use serde_json::{Value, json};

use super::{Doc, State, Status, Told};
use crate::capabilities;
use crate::error::Error;

/// What the server wants to hear about documents.
pub(crate) struct SyncOptions {
    pub(crate) open_close: bool,
    pub(crate) change: ChangeSync,
    pub(crate) save: SaveSync,
}

/// How the server wants edits sent, from LSP's `TextDocumentSyncKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeSync {
    None,
    FullText,
    Ranges,
}

impl ChangeSync {
    /// The kind numbered `kind` on the wire; numbers after 2 are read as ranges, as the protocol may add kinds.
    fn from_wire(kind: Option<u64>) -> ChangeSync {
        match kind {
            None | Some(0) => ChangeSync::None,
            Some(1) => ChangeSync::FullText,
            Some(_) => ChangeSync::Ranges,
        }
    }
}

/// Whether the server wants `didSave`, and whether with the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveSync {
    Never,
    WithoutText,
    WithText,
}

impl State {
    pub(crate) fn sync_options(&self) -> SyncOptions {
        match self.raw.get("textDocumentSync") {
            // The short form means: open and close, this kind of change, and saves without text (as VS Code does).
            Some(Value::Number(kind)) => SyncOptions {
                open_close: true,
                change: ChangeSync::from_wire(kind.as_u64()),
                save: SaveSync::WithoutText,
            },
            Some(options @ Value::Object(_)) => SyncOptions {
                open_close: options.get("openClose").and_then(Value::as_bool) == Some(true),
                change: ChangeSync::from_wire(options.get("change").and_then(Value::as_u64)),
                save: match options.get("save") {
                    Some(Value::Bool(true)) => SaveSync::WithoutText,
                    Some(save @ Value::Object(_)) if save.get("includeText").and_then(Value::as_bool) == Some(true) => {
                        SaveSync::WithText
                    }
                    Some(Value::Object(_)) => SaveSync::WithoutText,
                    _ => SaveSync::Never,
                },
            },
            _ => SyncOptions {
                open_close: false,
                change: ChangeSync::None,
                save: SaveSync::Never,
            },
        }
    }

    /// Whether the server offers `key` statically or registered `method`.
    pub(crate) fn offers(&self, key: &str, method: &str) -> bool {
        capabilities::offers(&self.raw, key) || self.registrations.values().any(|(m, _)| m == method)
    }

    /// The registration options of `method`, if registered.
    pub(crate) fn registered(&self, method: &str) -> Option<&Value> {
        self.registrations
            .values()
            .find(|(m, _)| m == method)
            .map(|(_, options)| options)
    }

    pub(crate) fn doc(&self, uri: &Uri) -> Result<&Doc, Error> {
        self.documents.get(uri).ok_or(Error::NotOpen)
    }

    pub(super) fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        if self.status != Status::Running {
            return Err(Error::NotRunning);
        }
        self.connection
            .as_ref()
            .ok_or(Error::NotRunning)?
            .notify(method, params)
    }

    /// Sends a notification whose parameters `build` makes on the connection's writer thread, in order with every
    /// other message: for those that carry a whole document.
    pub(super) fn notify_with(
        &self,
        method: &str,
        build: impl FnOnce() -> Value + Send + 'static,
    ) -> Result<(), Error> {
        if self.status != Status::Running {
            return Err(Error::NotRunning);
        }
        self.connection
            .as_ref()
            .ok_or(Error::NotRunning)?
            .notify_with(method, build)
    }

    pub(super) fn notify_document(&self, method: &str, params: Value) {
        if self.sync_options().open_close {
            let _ = self.notify(method, params);
        }
    }

    pub(super) fn notify_document_with(&self, method: &str, build: impl FnOnce() -> Value + Send + 'static) {
        if self.sync_options().open_close {
            let _ = self.notify_with(method, build);
        }
    }

    pub(super) fn send_changes(&mut self, uri: &Uri, changes: Vec<TextDocumentContentChangeEvent>) {
        let options = self.sync_options();
        let Some(doc) = self.documents.get(uri) else {
            return;
        };
        let document = json!({"uri": uri, "version": doc.version});
        let sent = match options.change {
            ChangeSync::None => return,
            ChangeSync::FullText => {
                let text = doc.text.clone();
                self.notify_with(
                    "textDocument/didChange",
                    move || json!({"textDocument": document, "contentChanges": [{"text": text.text()}]}),
                )
            }
            ChangeSync::Ranges => self.notify(
                "textDocument/didChange",
                json!({"textDocument": document, "contentChanges": changes}),
            ),
        };
        if sent.is_err()
            && let Some(doc) = self.documents.get_mut(uri)
        {
            doc.sync = Told::NeedsFullText;
        }
    }

    /// Sends what the server has not been told about `uri`, if it runs.
    pub(crate) fn flush(&mut self, uri: &Uri) {
        if self.status != Status::Running {
            return;
        }
        let options = self.sync_options();
        let Some(doc) = self.documents.get(uri) else {
            return;
        };
        let sent = match doc.sync {
            Told::Synced => return,
            Told::NeedsOpen if !options.open_close => true,
            Told::NeedsOpen => {
                let (document, text) = (
                    json!({"uri": uri, "languageId": doc.language, "version": doc.version}),
                    doc.text.clone(),
                );
                self.notify_with("textDocument/didOpen", move || {
                    let mut document = document;
                    document["text"] = Value::String(text.text());
                    json!({"textDocument": document})
                })
                .is_ok()
            }
            Told::NeedsFullText if options.change == ChangeSync::None => true,
            Told::NeedsFullText => {
                let (document, text) = (json!({"uri": uri, "version": doc.version}), doc.text.clone());
                self.notify_with(
                    "textDocument/didChange",
                    move || json!({"textDocument": document, "contentChanges": [{"text": text.text()}]}),
                )
                .is_ok()
            }
        };
        if sent && let Some(doc) = self.documents.get_mut(uri) {
            doc.sync = Told::Synced;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChangeSync;

    #[test]
    fn change_kinds_read_from_their_wire_numbers() {
        assert_eq!(ChangeSync::from_wire(None), ChangeSync::None);
        assert_eq!(ChangeSync::from_wire(Some(0)), ChangeSync::None);
        assert_eq!(ChangeSync::from_wire(Some(1)), ChangeSync::FullText);
        assert_eq!(ChangeSync::from_wire(Some(2)), ChangeSync::Ranges);
        assert_eq!(ChangeSync::from_wire(Some(7)), ChangeSync::Ranges);
    }
}
