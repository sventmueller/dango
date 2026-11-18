//! Open documents and the servers each is open on, so one call reaches every server of a document.

use ide_core::LockIgnoringPoison;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use lsp_types::{TextDocumentContentChangeEvent, Uri};

use crate::error::Error;
use crate::server::Server;
use crate::uri;

/// Which servers each open document is open on.
#[derive(Default)]
pub struct Documents {
    open: Mutex<HashMap<Uri, Vec<Server>>>,
}

impl Documents {
    /// No documents open.
    pub fn new() -> Documents {
        Documents::default()
    }

    /// Opens `uri` on each of `servers`; opened again, it is closed on servers no longer among them.
    pub fn open(&self, uri: &Uri, language_id: &str, version: i32, text: &str, servers: &[Server]) {
        let uri = uri::normalize(uri);
        for server in servers {
            server.open(&uri, language_id, version, text);
        }
        let before = self.lock().insert(uri.clone(), servers.to_vec()).unwrap_or_default();
        for old in before {
            if !servers.iter().any(|server| server.same(&old)) {
                old.close(&uri);
            }
        }
    }

    /// Passes `changes` to every server `uri` is open on; the first error is returned after all were told.
    pub fn change(&self, uri: &Uri, version: i32, changes: &[TextDocumentContentChangeEvent]) -> Result<(), Error> {
        let servers = self.servers(uri);
        if servers.is_empty() {
            return Err(Error::NotOpen);
        }
        let mut first = Ok(());
        for server in &servers {
            let result = server.change(uri, version, changes);
            if first.is_ok() {
                first = result;
            }
        }
        first
    }

    /// Tells every server of `uri` it was saved.
    pub fn save(&self, uri: &Uri) {
        for server in self.servers(uri) {
            server.save(uri);
        }
    }

    /// Closes `uri` on every server.
    pub fn close(&self, uri: &Uri) {
        let servers = self.lock().remove(&uri::normalize(uri)).unwrap_or_default();
        for server in servers {
            server.close(uri);
        }
    }

    /// The servers `uri` is open on, in the order given to `open`.
    pub fn servers(&self, uri: &Uri) -> Vec<Server> {
        self.lock().get(&uri::normalize(uri)).cloned().unwrap_or_default()
    }

    /// Forgets `server` for every document, for example before stopping it.
    pub fn detach(&self, server: &Server) {
        for servers in self.lock().values_mut() {
            servers.retain(|s| !s.same(server));
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Uri, Vec<Server>>> {
        self.open.lock_ignoring_poison()
    }
}
