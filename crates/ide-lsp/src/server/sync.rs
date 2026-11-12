//! Keeping the server in step with the open documents, as far as its capabilities ask for.

use lsp_types::{TextDocumentContentChangeEvent, Uri};
use serde_json::{Value, json};

use super::{Doc, State, Status, Told};
use crate::capabilities;
use crate::error::Error;

/// What the server wants to hear about documents.
pub(crate) struct SyncOptions {
    pub(crate) open_close: bool,
    /// 0 none, 1 the whole text, 2 the changed ranges.
    pub(crate) change: u64,
    /// Whether to send `didSave`, and whether with the text.
    pub(crate) save: Option<bool>,
}

impl State {
    pub(crate) fn sync_options(&self) -> SyncOptions {
        match self.raw.get("textDocumentSync") {
            // The short form means: open and close, this kind of change, and saves without text (as VS Code does).
            Some(Value::Number(kind)) => SyncOptions {
                open_close: true,
                change: kind.as_u64().unwrap_or(0),
                save: Some(false),
            },
            Some(options @ Value::Object(_)) => SyncOptions {
                open_close: options.get("openClose").and_then(Value::as_bool) == Some(true),
                change: options.get("change").and_then(Value::as_u64).unwrap_or(0),
                save: match options.get("save") {
                    Some(Value::Bool(true)) => Some(false),
                    Some(save @ Value::Object(_)) => {
                        Some(save.get("includeText").and_then(Value::as_bool) == Some(true))
                    }
                    _ => None,
                },
            },
            _ => SyncOptions {
                open_close: false,
                change: 0,
                save: None,
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

    pub(super) fn notify_document(&self, method: &str, params: Value) {
        if self.sync_options().open_close {
            let _ = self.notify(method, params);
        }
    }

    pub(super) fn send_changes(&mut self, uri: &Uri, changes: Vec<TextDocumentContentChangeEvent>) {
        let options = self.sync_options();
        let Some(doc) = self.documents.get(uri) else {
            return;
        };
        let changes = match options.change {
            0 => return,
            1 => vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: doc.text.text(),
            }],
            _ => changes,
        };
        let params = json!({
            "textDocument": {"uri": uri, "version": doc.version},
            "contentChanges": changes,
        });
        if self.notify("textDocument/didChange", params).is_err()
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
                let params = json!({"textDocument": {
                    "uri": uri, "languageId": doc.language, "version": doc.version, "text": doc.text.text(),
                }});
                self.notify("textDocument/didOpen", params).is_ok()
            }
            Told::NeedsFullText if options.change == 0 => true,
            Told::NeedsFullText => {
                let params = json!({
                    "textDocument": {"uri": uri, "version": doc.version},
                    "contentChanges": [{"text": doc.text.text()}],
                });
                self.notify("textDocument/didChange", params).is_ok()
            }
        };
        if sent && let Some(doc) = self.documents.get_mut(uri) {
            doc.sync = Told::Synced;
        }
    }
}
