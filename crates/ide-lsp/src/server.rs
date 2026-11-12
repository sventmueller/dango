//! One language server for one project root: started through the supervisor, initialized, kept in step with the
//! open documents, restarted after a crash and stopped with `shutdown` and `exit`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use ide_process::tail::Tail;
use ide_process::{Ending, Kind, ProcessId, Supervisor};
use lsp_types::{
    ApplyWorkspaceEditResponse, Diagnostic, MessageActionItem, MessageType, NumberOrString, Position, Range,
    ServerCapabilities, ShowDocumentResult, TextDocumentContentChangeEvent, Uri, WorkDoneProgress, WorkspaceEdit,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::capabilities;
use crate::convert::Positions;
use crate::encoding::Encoding;
use crate::error::Error;
use crate::rpc::{Connection, Handler, Responder};
use crate::semantic::Previous;
use crate::text::Text;
use crate::uri;

/// How much of a server's standard error is kept for the crash report.
const STDERR_KEPT: usize = 64 * 1024;

/// The largest file read from disk to recount positions in a document that is not open.
const LARGEST_UNOPENED: u64 = 32 * 1024 * 1024;

/// How to start a language server and how patient to be with it.
#[derive(Clone)]
pub struct ServerConfig {
    /// What the activity view shows, such as `gopls`.
    pub label: String,
    /// The server's executable.
    pub program: PathBuf,
    /// Its arguments, such as `--stdio`.
    pub args: Vec<String>,
    /// The whole environment, normally the login shell's; the app's own is not passed on.
    pub env: Vec<(String, String)>,
    /// The project root: the working directory and the one workspace folder.
    pub root: PathBuf,
    /// Sent with `initialize`.
    pub initialization_options: Option<Value>,
    /// The answer to `workspace/configuration`, looked up by section, such as `{"gopls": {...}}`.
    pub settings: Value,
    /// The position encodings offered, most wanted first.
    pub position_encodings: Vec<Encoding>,
    /// Whether the app watches files for the server; then servers register their watchers instead of watching
    /// themselves, and the app must report changes with `Server::watched_files_changed`.
    pub client_watches_files: bool,
    /// When to restart after a crash.
    pub restart: RestartPolicy,
    /// How long a request may take.
    pub request_timeout: Duration,
    /// How long `initialize` may take.
    pub start_timeout: Duration,
    /// How long each step of stopping may take: `shutdown`, then SIGTERM, then SIGKILL.
    pub stop_grace: Duration,
}

impl ServerConfig {
    /// A configuration with the usual limits.
    pub fn new(label: &str, program: &Path, root: &Path, env: Vec<(String, String)>) -> ServerConfig {
        ServerConfig {
            label: label.to_string(),
            program: program.to_path_buf(),
            args: vec![],
            env,
            root: root.to_path_buf(),
            initialization_options: None,
            settings: Value::Null,
            // rust-analyzer and typescript-go pick UTF-8 whenever it is offered, whatever the order, which would
            // recount every position; UTF-16 alone is what every server must accept.
            position_encodings: vec![Encoding::Utf16],
            client_watches_files: false,
            restart: RestartPolicy::default(),
            request_timeout: Duration::from_secs(30),
            start_timeout: Duration::from_secs(60),
            stop_grace: Duration::from_secs(2),
        }
    }
}

impl std::fmt::Debug for ServerConfig {
    /// Leaves out the environment's values: the login shell's environment often holds tokens.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("label", &self.label)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("env", &format_args!("{} variables", self.env.len()))
            .field("root", &self.root)
            .field("position_encodings", &self.position_encodings)
            .field("restart", &self.restart)
            .finish_non_exhaustive()
    }
}

/// When a crashed server is started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// Crashes in a row after which the server stays down.
    pub max_restarts: u32,
    /// The wait before the first restart; it doubles with every further crash in a row.
    pub first_delay: Duration,
    /// The longest wait.
    pub max_delay: Duration,
    /// A server that ran this long before it crashed starts the count anew.
    pub stable_after: Duration,
}

impl Default for RestartPolicy {
    fn default() -> RestartPolicy {
        RestartPolicy {
            max_restarts: 5,
            first_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            stable_after: Duration::from_secs(120),
        }
    }
}

impl RestartPolicy {
    /// The wait before restart number `attempt`, counted from 1.
    pub fn delay(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        self.first_delay.saturating_mul(factor).min(self.max_delay)
    }
}

/// Where a server is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Started; `initialize` has not been answered yet.
    Starting,
    /// Answering requests.
    Running,
    /// Crashed and waiting `delay` before restart number `attempt`.
    Restarting {
        /// The restart's number in the current run of crashes; 0 for a restart someone asked for.
        attempt: u32,
        /// The wait.
        delay: Duration,
    },
    /// Asked to stop and not yet gone.
    Stopping,
    /// Stopped on request.
    Stopped,
    /// Crashed too often in a row and stays down until restarted by hand.
    Failed,
}

/// What a server reported or asks of the app; delivered on a background thread.
#[derive(Debug)]
pub enum Event {
    /// The server's status changed.
    Status(Status),
    /// The process ended without being asked to.
    Crashed {
        /// How it ended, if the supervisor told in time.
        ending: Option<Ending>,
        /// The end of what it wrote to standard error.
        stderr: String,
        /// Whether a restart follows.
        restarting: bool,
    },
    /// The diagnostics of a document, replacing earlier ones.
    Diagnostics {
        /// The document.
        uri: Uri,
        /// The document version they belong to, if the server said.
        version: Option<i32>,
        /// With UTF-16 columns.
        diagnostics: Vec<Diagnostic>,
    },
    /// Progress of work the server does on its own, such as indexing.
    Progress {
        /// Tells apart simultaneous pieces of work.
        token: NumberOrString,
        /// Begin, report or end.
        progress: WorkDoneProgress,
    },
    /// A line for the server's log.
    Log {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
    },
    /// A message to show the user.
    Message {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
    },
    /// A message to show the user with buttons; the reply is the chosen one, or `None` when dismissed.
    MessageRequest {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
        /// The buttons.
        actions: Vec<MessageActionItem>,
        /// Must be answered; dropping it answers `None`.
        reply: Reply<Option<MessageActionItem>>,
    },
    /// The server wants the app to apply edits, for example after a code action's command.
    ApplyEdit {
        /// What the user may read in the undo menu.
        label: Option<String>,
        /// With UTF-16 columns.
        edit: WorkspaceEdit,
        /// Must be answered; dropping it answers that nothing was applied.
        reply: Reply<ApplyWorkspaceEditResponse>,
    },
    /// The server wants a document or URL shown.
    ShowDocument {
        /// What to show.
        uri: Uri,
        /// Whether it is meant for the system's browser.
        external: bool,
        /// Whether the editor should take focus.
        take_focus: bool,
        /// What to select, with UTF-16 columns.
        selection: Option<Range>,
        /// Dropping it answers that nothing was shown.
        reply: Reply<ShowDocumentResult>,
    },
    /// The server registered or unregistered capabilities; features may have come or gone.
    CapabilitiesChanged,
    /// Results of this kind are out of date and should be asked for again.
    Refresh(Refresh),
}

/// A kind of result the server declared out of date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    /// Semantic tokens.
    SemanticTokens,
    /// Inlay hints.
    InlayHints,
    /// Pulled diagnostics.
    Diagnostics,
    /// Code lenses.
    CodeLenses,
    /// Folding ranges.
    FoldingRanges,
}

/// The answer the server waits for; sent once, or the fallback when dropped.
pub struct Reply<T: Serialize> {
    responder: Option<Responder>,
    fallback: Option<T>,
}

impl<T: Serialize> Reply<T> {
    pub(crate) fn new(responder: Responder, fallback: T) -> Reply<T> {
        Reply {
            responder: Some(responder),
            fallback: Some(fallback),
        }
    }

    /// Answers with `answer`.
    pub fn send(mut self, answer: T) {
        if let Some(responder) = self.responder.take() {
            responder.ok(serde_json::to_value(answer).unwrap_or(Value::Null));
        }
    }
}

impl<T: Serialize> Drop for Reply<T> {
    fn drop(&mut self) {
        if let (Some(responder), Some(fallback)) = (self.responder.take(), self.fallback.take()) {
            responder.ok(serde_json::to_value(fallback).unwrap_or(Value::Null));
        }
    }
}

impl<T: Serialize> std::fmt::Debug for Reply<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.responder.is_some() {
            "Reply(waiting)"
        } else {
            "Reply(sent)"
        })
    }
}

/// A request on its way; `cancel` ends it with `Error::Cancelled` and tells the server.
#[derive(Debug, Clone)]
pub struct Ticket {
    connection: Weak<Connection>,
    id: i32,
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
    /// Shared with conversions in flight; an edit copies it only while one runs.
    pub(crate) text: Arc<Text>,
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
    pub(crate) capabilities: Option<ServerCapabilities>,
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

    /// What the server offers, once it is running.
    pub fn capabilities(&self) -> Option<ServerCapabilities> {
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
            state = inner
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
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
                text: Arc::new(Text::new(text)),
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
        let text = Arc::make_mut(&mut doc.text);
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
        let mut params = json!({"textDocument": {"uri": uri}});
        if include_text {
            params["text"] = Value::String(doc.text.text());
        }
        state.notify_document("textDocument/didSave", params);
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

/// The method names the crate asks for; an error names one without allocating.
fn static_name(method: &str) -> &'static str {
    crate::features::METHODS
        .iter()
        .find(|m| **m == method)
        .copied()
        .unwrap_or("this request")
}

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

    /// `position` in `uri`, from UTF-16 to the server's unit.
    pub(crate) fn to_server(&self, uri: &Uri, position: Position) -> Result<Position, Error> {
        Ok(self.doc(uri)?.text.convert(position, Encoding::Utf16, self.encoding))
    }

    /// Converts every position in `value` from UTF-16 to the server's unit; documents not open stay as they are.
    pub(crate) fn positions_to_server<T: Positions>(&self, uri: &Uri, mut value: T) -> T {
        if self.encoding != Encoding::Utf16 {
            let encoding = self.encoding;
            value.each(uri, &mut |u, p| {
                if let Some(doc) = self.documents.get(&uri::normalize(u)) {
                    *p = doc.text.convert(*p, Encoding::Utf16, encoding);
                }
            });
        }
        value
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        if self.status != Status::Running {
            return Err(Error::NotRunning);
        }
        self.connection
            .as_ref()
            .ok_or(Error::NotRunning)?
            .notify(method, params)
    }

    fn notify_document(&self, method: &str, params: Value) {
        if self.sync_options().open_close {
            let _ = self.notify(method, params);
        }
    }

    fn send_changes(&mut self, uri: &Uri, changes: Vec<TextDocumentContentChangeEvent>) {
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

impl Inner {
    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A converter of positions in answers from the server's unit to UTF-16.
    pub(crate) fn mapper(self: &Arc<Self>) -> Mapper {
        Mapper {
            encoding: self.lock().encoding,
            inner: Arc::downgrade(self),
            texts: HashMap::new(),
        }
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.sink)(event)));
    }

    fn set_status(&self, state: &mut State, status: Status) {
        state.status = status;
        self.changed.notify_all();
    }

    /// Starts a process and sends `initialize`.
    fn launch(self: &Arc<Self>) -> io::Result<()> {
        let config = &self.config;
        let (connection, generation) = {
            let mut state = self.lock();
            let mut command = Command::new(&config.program);
            command
                .args(&config.args)
                .current_dir(&config.root)
                .env_clear()
                .envs(config.env.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let spawned = self
                .supervisor
                .spawn(command, Kind::LanguageServer, config.label.clone())?;
            let (Some(stdin), Some(stdout), Some(stderr)) = (spawned.stdin, spawned.stdout, spawned.stderr) else {
                self.supervisor.stop(spawned.id, config.stop_grace);
                return Err(io::Error::other("the server's pipes are missing"));
            };
            state.generation += 1;
            let generation = state.generation;
            let tail = Arc::new(Mutex::new(Tail::new(STDERR_KEPT)));
            let _ = ide_process::tail::collect(stderr, tail.clone());
            let hook = Arc::new(Hook {
                inner: Arc::downgrade(self),
                generation,
            });
            let connection = match Connection::start(stdout, stdin, hook, &config.label) {
                Ok(connection) => connection,
                Err(error) => {
                    self.supervisor.stop(spawned.id, config.stop_grace);
                    return Err(error);
                }
            };
            self.supervisor.on_stop(
                spawned.id,
                graceful(Arc::downgrade(self), connection.clone(), generation, config.stop_grace),
            );
            state.process = Some(spawned.id);
            state.connection = Some(connection.clone());
            state.stderr = tail;
            state.capabilities = None;
            state.raw = Value::Null;
            state.encoding = Encoding::Utf16;
            state.registrations.clear();
            for doc in state.documents.values_mut() {
                doc.sync = Told::NeedsOpen;
                doc.tokens = None;
                doc.pulled = None;
            }
            self.set_status(&mut state, Status::Starting);
            (connection, generation)
        };
        self.emit(Event::Status(Status::Starting));
        let weak = Arc::downgrade(self);
        let sent = connection.request(
            "initialize",
            self.initialize_params(),
            config.start_timeout,
            Box::new(move |answer| {
                if let Some(inner) = weak.upgrade() {
                    inner.initialized(generation, answer);
                }
            }),
        );
        if sent.is_err() {
            self.gone(generation);
        }
        Ok(())
    }

    fn initialize_params(&self) -> Value {
        let root = uri::from_path(&self.config.root);
        let name = self
            .config
            .root
            .file_name()
            .map_or_else(|| "root".to_string(), |n| n.to_string_lossy().into_owned());
        let mut params = json!({
            "processId": std::process::id(),
            "clientInfo": {"name": ide_product::NAME},
            "rootUri": root,
            "workspaceFolders": [{"uri": root, "name": name}],
            "capabilities": capabilities::client(&self.config.position_encodings, self.config.client_watches_files),
            "trace": "off",
        });
        if let Some(options) = &self.config.initialization_options {
            params["initializationOptions"] = options.clone();
        }
        params
    }

    fn initialized(self: &Arc<Self>, generation: u64, answer: Result<Value, Error>) {
        let Ok(result) = answer else {
            self.gone(generation);
            return;
        };
        {
            let mut state = self.lock();
            if state.generation != generation || state.status != Status::Starting {
                return;
            }
            let raw = result.get("capabilities").cloned().unwrap_or(Value::Null);
            state.capabilities = serde_json::from_value(raw.clone()).ok();
            state.encoding = Encoding::from_kind(
                raw.get("positionEncoding")
                    .and_then(Value::as_str)
                    .map(|s| lsp_types::PositionEncodingKind::from(s.to_string()))
                    .as_ref(),
            );
            state.raw = raw;
            let Some(connection) = state.connection.clone() else {
                return;
            };
            let _ = connection.notify("initialized", json!({}));
            if !state.settings.is_null() {
                let _ = connection.notify("workspace/didChangeConfiguration", json!({"settings": state.settings}));
            }
            state.running_since = Some(Instant::now());
            self.set_status(&mut state, Status::Running);
            let uris: Vec<Uri> = state.documents.keys().cloned().collect();
            for uri in uris {
                state.flush(&uri);
            }
        }
        self.emit(Event::Status(Status::Running));
    }

    /// The process of `generation` ended, or never got going: stopped as asked, or crashed.
    fn gone(self: &Arc<Self>, generation: u64) {
        let (process, stderr, restart) = {
            let mut state = self.lock();
            if state.generation != generation {
                return;
            }
            match state.status {
                Status::Stopping | Status::Stopped => {
                    state.process = None;
                    state.connection = None;
                    let was = state.status;
                    self.set_status(&mut state, Status::Stopped);
                    drop(state);
                    if was == Status::Stopping {
                        self.emit(Event::Status(Status::Stopped));
                    }
                    return;
                }
                Status::Restarting { .. } | Status::Failed => return,
                Status::Starting | Status::Running => {}
            }
            let policy = self.config.restart;
            if state.running_since.is_some_and(|t| t.elapsed() >= policy.stable_after) {
                state.crashes = 0;
            }
            state.crashes += 1;
            state.running_since = None;
            state.connection = None;
            let restart = (state.crashes <= policy.max_restarts).then(|| policy.delay(state.crashes));
            let status = restart.map_or(Status::Failed, |delay| Status::Restarting {
                attempt: state.crashes,
                delay,
            });
            self.set_status(&mut state, status);
            (state.process.take(), state.stderr.clone(), restart)
        };
        let inner = self.clone();
        let _ = std::thread::Builder::new()
            .name(format!("lsp-restart-{}", self.config.label))
            .spawn(move || inner.recover(generation, process, &stderr, restart));
    }

    /// Reports a crash, waits, and starts the server again unless it was stopped meanwhile.
    fn recover(
        self: &Arc<Self>,
        generation: u64,
        process: Option<ProcessId>,
        stderr: &Mutex<Tail>,
        delay: Option<Duration>,
    ) {
        let grace = self.config.stop_grace;
        let ending = process.and_then(|id| {
            self.supervisor.stop(id, grace);
            self.exits.wait(id, grace * 3 + Duration::from_millis(500))
        });
        let stderr = stderr.lock().unwrap_or_else(|e| e.into_inner()).text();
        self.emit(Event::Crashed {
            ending,
            stderr,
            restarting: delay.is_some(),
        });
        let status = self.lock().status;
        self.emit(Event::Status(status));
        let Some(delay) = delay else {
            return;
        };
        let deadline = Instant::now() + delay;
        let mut state = self.lock();
        loop {
            if state.generation != generation || !matches!(state.status, Status::Restarting { .. }) {
                return;
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        drop(state);
        if self.launch().is_err() {
            // A launch that fails counts as a crash of a process that never ran.
            let mut state = self.lock();
            state.status = Status::Starting;
            let generation = state.generation;
            drop(state);
            self.gone(generation);
        }
    }

    fn stop(self: &Arc<Self>) {
        let process = {
            let mut state = self.lock();
            match state.status {
                Status::Stopped | Status::Stopping => return,
                Status::Failed | Status::Restarting { .. } if state.process.is_none() => {
                    self.set_status(&mut state, Status::Stopped);
                    drop(state);
                    self.emit(Event::Status(Status::Stopped));
                    return;
                }
                _ => {}
            }
            self.set_status(&mut state, Status::Stopping);
            state.process
        };
        self.emit(Event::Status(Status::Stopping));
        match process {
            Some(id) => self.supervisor.stop(id, self.config.stop_grace),
            None => {
                let mut state = self.lock();
                self.set_status(&mut state, Status::Stopped);
                drop(state);
                self.emit(Event::Status(Status::Stopped));
            }
        }
    }

    fn restart(self: &Arc<Self>) {
        let (process, generation) = {
            let mut state = self.lock();
            state.crashes = 0;
            state.running_since = None;
            state.connection = None;
            self.set_status(
                &mut state,
                Status::Restarting {
                    attempt: 0,
                    delay: Duration::ZERO,
                },
            );
            (state.process.take(), state.generation)
        };
        self.emit(Event::Status(Status::Restarting {
            attempt: 0,
            delay: Duration::ZERO,
        }));
        let inner = self.clone();
        let _ = std::thread::Builder::new()
            .name(format!("lsp-restart-{}", self.config.label))
            .spawn(move || {
                if let Some(id) = process {
                    inner.supervisor.stop(id, inner.config.stop_grace);
                    inner
                        .exits
                        .wait(id, inner.config.stop_grace * 3 + Duration::from_millis(500));
                }
                let current = inner.lock();
                if current.generation != generation || !matches!(current.status, Status::Restarting { .. }) {
                    return;
                }
                drop(current);
                if inner.launch().is_err() {
                    let mut state = inner.lock();
                    state.status = Status::Starting;
                    let generation = state.generation;
                    drop(state);
                    inner.gone(generation);
                }
            });
    }

    /// The answer to `workspace/configuration`: each item's section looked up in the settings.
    pub(crate) fn configuration(&self, params: &Value) -> Value {
        let settings = self.lock().settings.clone();
        let items = params
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Value::Array(
            items
                .iter()
                .map(|item| match item.get("section").and_then(Value::as_str) {
                    None | Some("") => settings.clone(),
                    Some(section) => section
                        .split('.')
                        .try_fold(&settings, |value, key| value.get(key))
                        .cloned()
                        .unwrap_or(Value::Null),
                })
                .collect(),
        )
    }
}

/// The supervisor's first step when the process is stopped: `shutdown`, wait for its answer, `exit`.
fn graceful(inner: Weak<Inner>, connection: Arc<Connection>, generation: u64, grace: Duration) -> impl FnOnce() + Send {
    move || {
        if let Some(inner) = inner.upgrade() {
            let mut state = inner.lock();
            if state.generation == generation && matches!(state.status, Status::Starting | Status::Running) {
                inner.set_status(&mut state, Status::Stopping);
                drop(state);
                inner.emit(Event::Status(Status::Stopping));
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let asked = connection.request(
            "shutdown",
            Value::Null,
            grace,
            Box::new(move |answer| {
                let _ = tx.send(answer);
            }),
        );
        if asked.is_ok() {
            let _ = rx.recv_timeout(grace);
        }
        let _ = connection.notify("exit", Value::Null);
        connection.end_input();
    }
}

/// Recounts positions in answers from the server's unit to UTF-16, reading documents that are not open from disk.
pub(crate) struct Mapper {
    encoding: Encoding,
    inner: Weak<Inner>,
    texts: HashMap<Uri, Option<Arc<Text>>>,
}

impl Mapper {
    pub(crate) fn recount<T: Positions>(&mut self, uri: &Uri, value: &mut T) {
        if self.encoding == Encoding::Utf16 {
            return;
        }
        let encoding = self.encoding;
        let (inner, texts) = (&self.inner, &mut self.texts);
        value.each(uri, &mut |u, p| {
            let text = texts.entry(u.clone()).or_insert_with(|| load(inner, u));
            if let Some(text) = text {
                *p = text.convert(*p, encoding, Encoding::Utf16);
            }
        });
    }
}

fn load(inner: &Weak<Inner>, uri: &Uri) -> Option<Arc<Text>> {
    let key = uri::normalize(uri);
    if let Some(inner) = inner.upgrade()
        && let Some(doc) = inner.lock().documents.get(&key)
    {
        return Some(doc.text.clone());
    }
    let path = uri::to_path(&key)?;
    if std::fs::metadata(&path).ok()?.len() > LARGEST_UNOPENED {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(Arc::new(Text::new(&String::from_utf8_lossy(&bytes))))
}

/// Hears one process's connection; knows its generation so that news from an older process is dropped.
struct Hook {
    inner: Weak<Inner>,
    generation: u64,
}

impl Handler for Hook {
    fn notification(&self, method: &str, params: Value) {
        if let Some(inner) = self.inner.upgrade() {
            crate::incoming::notification(&inner, method, params);
        }
    }

    fn request(&self, method: &str, params: Value, responder: Responder) {
        match self.inner.upgrade() {
            Some(inner) => crate::incoming::request(&inner, method, params, responder),
            None => responder.fail(-32800, "the client closed the server"),
        }
    }

    fn closed(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.gone(self.generation);
        }
    }
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

    #[test]
    fn a_configuration_does_not_print_the_environment() {
        let env = vec![("GITHUB_TOKEN".to_string(), "secret".to_string())];
        let config = ServerConfig::new("x", Path::new("/bin/x"), Path::new("/"), env);
        let printed = format!("{config:?}");
        assert!(!printed.contains("secret") && printed.contains("1 variables"));
    }

    #[test]
    fn restarts_wait_longer_each_time_up_to_the_limit() {
        let policy = RestartPolicy {
            max_restarts: 10,
            first_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
            stable_after: Duration::from_secs(1),
        };
        let delays: Vec<u128> = (1..=6).map(|n| policy.delay(n).as_millis()).collect();
        assert_eq!(delays, [100, 200, 400, 800, 1000, 1000]);
        assert_eq!(policy.delay(u32::MAX), policy.max_delay);
    }
}
