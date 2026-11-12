//! A server's process: launch and `initialize`, crashes and restarts, and the graceful stop.

use std::io;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use ide_process::tail::Tail;
use ide_process::{Kind, ProcessId};
use lsp_types::Uri;
use serde_json::{Value, json};

use super::{Event, Inner, STDERR_KEPT, State, Status, Told};
use crate::capabilities;
use crate::encoding::Encoding;
use crate::error::Error;
use crate::rpc::{Connection, Handler, Responder};
use crate::uri;

impl Inner {
    fn set_status(&self, state: &mut State, status: Status) {
        state.status = status;
        self.changed.notify_all();
    }

    /// Starts a process and sends `initialize`.
    pub(super) fn launch(self: &Arc<Self>) -> io::Result<()> {
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

    pub(super) fn stop(self: &Arc<Self>) {
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

    pub(super) fn restart(self: &Arc<Self>) {
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
