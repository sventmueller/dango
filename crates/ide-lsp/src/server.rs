//! One language server for one project root: started through the supervisor, initialized, kept in step with the
//! open documents, restarted after a crash and stopped with `shutdown` and `exit`.

mod config;
mod event;
mod lifecycle;
mod positions;
mod sync;

use ide_core::{IgnoringPoison, LockIgnoringPoison};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use ide_process::tail::Tail;
use ide_process::{ProcessId, Supervisor};
use lsp_types::{Diagnostic, Range, ServerCapabilities, TextDocumentContentChangeEvent, Uri};
use serde_json::{Value, json};

use crate::encoding::Encoding;
use crate::error::Error;
use crate::rpc::Connection;
use crate::semantic::Previous;
use crate::text::Text;
use crate::uri;

pub use config::{RestartPolicy, ServerConfig};
pub use event::{Event, Refresh, Reply, Status};
pub(crate) use positions::Mapper;

/// How much of a server's standard error is kept for the crash report.
const STDERR_KEPT: usize = 64 * 1024;

/// A request on its way; `cancel` ends it with `Error::Cancelled` and tells the server.
#[derive(Debug, Clone)]
pub struct Ticket {
    connection: Weak<Connection>,
    id: i64,
}

impl Ticket {
    /// Cancels the request unless it was answered already.
    pub fn cancel(&self) {
        if let Some(connection) = self.connection.upgrade() {
            connection.cancel(self.id, Error::Cancelled);
        }
    }
}

/// Waits for a request started with `start`; for background threads and tests, never for the main thread.
pub fn wait<T: Send + 'static>(
    start: impl FnOnce(Box<dyn FnOnce(Result<T, Error>) + Send>) -> Result<Ticket, Error>,
) -> Result<T, Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    start(Box::new(move |answer| {
        let _ = tx.send(answer);
    }))?;
    rx.recv().unwrap_or(Err(Error::ServerGone))
}

type Sink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Told {
    /// The server knows the document as the client does.
    Synced,
    /// The server has not been told the document is open.
    NeedsOpen,
    /// A change could not be sent; the next one sends the whole text.
    NeedsFullText,
}

pub(crate) struct Doc {
    pub(crate) language: String,
    pub(crate) version: i32,
    pub(crate) text: Text,
    pub(crate) sync: Told,
    pub(crate) tokens: Option<Previous>,
    pub(crate) pulled: Option<(String, Vec<Diagnostic>)>,
}

pub(crate) struct State {
    pub(crate) status: Status,
    /// Counts processes, so that news from an earlier one is ignored.
    generation: u64,
    process: Option<ProcessId>,
    pub(crate) connection: Option<Arc<Connection>>,
    stderr: Arc<Mutex<Tail>>,
    pub(crate) capabilities: Option<Arc<ServerCapabilities>>,
    /// The capabilities as sent, to look up what lsp-types does not model.
    pub(crate) raw: Value,
    pub(crate) encoding: Encoding,
    pub(crate) documents: HashMap<Uri, Doc>,
    pub(crate) registrations: HashMap<String, (String, Value)>,
    settings: Value,
    crashes: u32,
    running_since: Option<Instant>,
}

pub(crate) struct Inner {
    pub(crate) config: ServerConfig,
    supervisor: Supervisor,
    exits: crate::tool::Exits,
    sink: Sink,
    state: Mutex<State>,
    changed: Condvar,
}

/// The external handle; when the last one goes, the server is stopped.
struct Handle {
    inner: Arc<Inner>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

/// A language server; clones share it, and dropping the last one stops it.
///
/// Positions and ranges in and out are in UTF-16 code units, whatever the server counts in. Calls do not block:
/// requests answer through a callback on a background thread, and document changes are queued.
#[derive(Clone)]
pub struct Server {
    handle: Arc<Handle>,
}

impl Server {
    /// Starts the server; `events` hears what it reports, on a background thread, and must return quickly.
    ///
    /// The caller has decided that the project is trusted: a server runs code the project controls.
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use ide_lsp::lsp_types::Position;
    /// use ide_lsp::{Server, ServerConfig, uri, wait};
    /// use ide_process::Supervisor;
    ///
    /// let root = Path::new("/work/hello");
    /// let env: Vec<(String, String)> = std::env::vars().collect();
    /// let config = ServerConfig::new("gopls", Path::new("/opt/homebrew/bin/gopls"), root, env);
    /// let server = Server::start(Supervisor::global(), config, |_event| {})?;
    /// let file = uri::from_path(&root.join("main.go"));
    /// server.open(&file, "go", 1, "package main\n\nfunc main() {}\n");
    /// let hover = wait(|reply| server.hover(&file, Position::new(2, 6), reply))?;
    /// assert!(hover.is_some_and(|hover| hover.markdown.contains("main")));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn start(
        supervisor: &Supervisor,
        config: ServerConfig,
        events: impl Fn(Event) + Send + Sync + 'static,
    ) -> io::Result<Server> {
        let settings = config.settings.clone();
        let inner = Arc::new(Inner {
            config,
            supervisor: supervisor.clone(),
            exits: crate::tool::Exits::watch(supervisor),
            sink: Arc::new(events),
            state: Mutex::new(State {
                status: Status::Stopped,
                generation: 0,
                process: None,
                connection: None,
                stderr: Arc::new(Mutex::new(Tail::new(STDERR_KEPT))),
                capabilities: None,
                raw: Value::Null,
                encoding: Encoding::Utf16,
                documents: HashMap::new(),
                registrations: HashMap::new(),
                settings,
                crashes: 0,
                running_since: None,
            }),
            changed: Condvar::new(),
        });
        inner.launch()?;
        Ok(Server {
            handle: Arc::new(Handle { inner }),
        })
    }

    pub(crate) fn inner(&self) -> &Arc<Inner> {
        &self.handle.inner
    }

    /// Whether `other` is a handle of the same server.
    pub fn same(&self, other: &Server) -> bool {
        Arc::ptr_eq(&self.handle, &other.handle)
    }

    /// What the activity view shows.
    pub fn label(&self) -> &str {
        &self.inner().config.label
    }

    /// The project root.
    pub fn root(&self) -> &Path {
        &self.inner().config.root
    }

    /// Where the server is in its life.
    pub fn status(&self) -> Status {
        self.inner().lock().status
    }

    /// The supervisor's number for the current process.
    pub fn process(&self) -> Option<ProcessId> {
        self.inner().lock().process
    }

    /// What the server offers, once it is running; a copy, see [`Server::shared_capabilities`].
    pub fn capabilities(&self) -> Option<ServerCapabilities> {
        self.shared_capabilities().map(|c| (*c).clone())
    }

    /// What the server offers, once it is running, without copying it: for checks on every keystroke.
    pub fn shared_capabilities(&self) -> Option<Arc<ServerCapabilities>> {
        self.inner().lock().capabilities.clone()
    }

    /// The unit the server counts columns in; the API converts to and from UTF-16 either way.
    pub fn encoding(&self) -> Encoding {
        self.inner().lock().encoding
    }

    /// Waits until the server runs or `timeout` passed; for background threads and tests.
    pub fn wait_until_running(&self, timeout: Duration) -> bool {
        self.wait_for(timeout, |status| status == Status::Running)
    }

    /// Waits until `done` holds for the status or `timeout` passed; for background threads and tests.
    pub fn wait_for(&self, timeout: Duration, done: impl Fn(Status) -> bool) -> bool {
        let inner = self.inner();
        let deadline = Instant::now() + timeout;
        let mut state = inner.lock();
        loop {
            if done(state.status) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = inner.changed.wait_timeout(state, deadline - now).ignoring_poison().0;
        }
    }

    /// Stops the server: `shutdown`, `exit`, then signals if it does not go.
    pub fn stop(&self) {
        self.inner().stop();
    }

    /// Starts the server anew, also after it stopped or failed; open documents are opened again.
    pub fn restart(&self) {
        self.inner().restart();
    }

    /// Opens `uri` with `text` at `version`; opening an open document replaces it.
    pub fn open(&self, uri: &Uri, language_id: &str, version: i32, text: &str) {
        let uri = uri::normalize(uri);
        let mut state = self.inner().lock();
        let replaced = state.documents.remove(&uri);
        if replaced.is_some_and(|doc| doc.sync != Told::NeedsOpen) {
            state.notify_document("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
        }
        state.documents.insert(
            uri.clone(),
            Doc {
                language: language_id.to_string(),
                version,
                text: Text::new(text),
                sync: Told::NeedsOpen,
                tokens: None,
                pulled: None,
            },
        );
        state.flush(&uri);
    }

    /// Applies `changes` to `uri`, in order, each range in UTF-16 against the text after the change before it, as
    /// `textDocument/didChange` defines; a change without a range replaces the whole text.
    pub fn change(&self, uri: &Uri, version: i32, changes: &[TextDocumentContentChangeEvent]) -> Result<(), Error> {
        let uri = uri::normalize(uri);
        let mut state = self.inner().lock();
        let encoding = state.encoding;
        let doc = state.documents.get_mut(&uri).ok_or(Error::NotOpen)?;
        let text = &mut doc.text;
        let mut sent = Vec::with_capacity(changes.len());
        for change in changes {
            if let Some(range) = change.range {
                let start = text.clamp(range.start);
                let end = text.clamp(range.end);
                let server = Range::new(
                    text.convert(start, Encoding::Utf16, encoding),
                    text.convert(end, Encoding::Utf16, encoding),
                );
                text.replace(Range::new(start, end), &change.text, Encoding::Utf16);
                sent.push(TextDocumentContentChangeEvent {
                    range: Some(server),
                    range_length: None,
                    text: change.text.clone(),
                });
            } else {
                *text = Text::new(&change.text);
                sent.push(change.clone());
            }
        }
        doc.version = version;
        if doc.sync != Told::Synced {
            state.flush(&uri);
            return Ok(());
        }
        state.send_changes(&uri, sent);
        Ok(())
    }

    /// Tells the server `uri` was saved.
    pub fn save(&self, uri: &Uri) {
        let uri = uri::normalize(uri);
        let state = self.inner().lock();
        let Some(include_text) = state.sync_options().save else {
            return;
        };
        let Some(doc) = state.documents.get(&uri).filter(|doc| doc.sync == Told::Synced) else {
            return;
        };
        if include_text {
            let text = doc.text.clone();
            state.notify_document_with(
                "textDocument/didSave",
                move || json!({"textDocument": {"uri": uri}, "text": text.text()}),
            );
        } else {
            state.notify_document("textDocument/didSave", json!({"textDocument": {"uri": uri}}));
        }
    }

    /// Closes `uri`.
    pub fn close(&self, uri: &Uri) {
        let uri = uri::normalize(uri);
        let mut state = self.inner().lock();
        if state
            .documents
            .remove(&uri)
            .is_some_and(|doc| doc.sync != Told::NeedsOpen)
        {
            state.notify_document("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
        }
    }

    /// Whether `uri` is open on this server.
    pub fn is_open(&self, uri: &Uri) -> bool {
        self.inner().lock().documents.contains_key(&uri::normalize(uri))
    }

    /// The text of `uri` as the server was told it.
    pub fn text(&self, uri: &Uri) -> Option<String> {
        self.inner()
            .lock()
            .documents
            .get(&uri::normalize(uri))
            .map(|doc| doc.text.text())
    }

    /// Replaces the settings answered to `workspace/configuration` and tells the server.
    pub fn did_change_configuration(&self, settings: Value) -> Result<(), Error> {
        let mut state = self.inner().lock();
        state.settings = settings.clone();
        state.notify("workspace/didChangeConfiguration", json!({"settings": settings}))
    }

    /// Reports file changes the app's watcher saw, when `client_watches_files` is set.
    pub fn watched_files_changed(&self, changes: Vec<lsp_types::FileEvent>) -> Result<(), Error> {
        let state = self.inner().lock();
        state.notify("workspace/didChangeWatchedFiles", json!({"changes": changes}))
    }

    /// The file watchers the server registered, as `DidChangeWatchedFilesRegistrationOptions`.
    pub fn watchers(&self) -> Vec<Value> {
        self.inner()
            .lock()
            .registrations
            .values()
            .filter(|(method, _)| method == "workspace/didChangeWatchedFiles")
            .map(|(_, options)| options.clone())
            .collect()
    }

    /// Sends any request; positions in it and in the answer are not converted.
    pub fn request_value(
        &self,
        method: &str,
        params: Value,
        reply: impl FnOnce(Result<Value, Error>) + Send + 'static,
    ) -> Result<Ticket, Error> {
        let method = method.to_string();
        self.call(&method, None, |_| Ok(params), Box::new(reply))
    }

    /// Sends any notification; positions in it are not converted.
    pub fn notify_value(&self, method: &str, params: Value) -> Result<(), Error> {
        self.inner().lock().notify(method, params)
    }

    /// Sends request `method` if the server runs and offers `capability` (a provider key in its capabilities,
    /// or a registration of `method`); `build` makes the parameters from the state.
    pub(crate) fn call(
        &self,
        method: &str,
        capability: Option<&str>,
        build: impl FnOnce(&State) -> Result<Value, Error>,
        reply: crate::rpc::Reply,
    ) -> Result<Ticket, Error> {
        let (connection, params) = {
            let state = self.inner().lock();
            if state.status != Status::Running {
                return Err(Error::NotRunning);
            }
            if let Some(capability) = capability
                && !state.offers(capability, method)
            {
                return Err(Error::Unsupported(static_name(method)));
            }
            let params = build(&state)?;
            (state.connection.clone().ok_or(Error::NotRunning)?, params)
        };
        let timeout = self.inner().config.request_timeout;
        let id = connection.request(method, params, timeout, reply)?;
        Ok(Ticket {
            connection: Arc::downgrade(&connection),
            id,
        })
    }
}

impl Inner {
    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock_ignoring_poison()
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.sink)(event)));
    }
}

/// The method names the crate asks for; an error names one without allocating.
fn static_name(method: &str) -> &'static str {
    crate::features::METHODS
        .iter()
        .find(|m| **m == method)
        .copied()
        .unwrap_or("this request")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send<T: Send>() {}
    fn shared<T: Send + Sync>() {}

    #[test]
    fn handles_and_events_can_cross_threads() {
        shared::<Server>();
        shared::<Ticket>();
        send::<Event>();
    }
}
