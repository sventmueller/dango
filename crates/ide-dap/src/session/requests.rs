//! The requests the app sends while a session runs: inspecting a stopped program, stepping, breakpoints.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{Event, Inner, SHORT, Session};
use crate::convert;
use crate::error::Error;
use crate::types::{
    Breakpoint, EvaluateContext, Evaluation, FrameId, Reply, Scope, SourceBreakpoint, StackFrame, State, Thread,
    ThreadId, Variable, VariablesRef,
};

/// Sends `command` and hands the body, turned by `convert`, to `reply`.
pub(crate) fn send<T: Send + 'static>(
    inner: &Inner,
    command: &str,
    arguments: Value,
    convert: impl FnOnce(&Value) -> T + Send + 'static,
    reply: Reply<T>,
) {
    let connection = match inner.connection() {
        Ok(connection) => connection,
        Err(error) => return reply(Err(error)),
    };
    let slot = Arc::new(Mutex::new(Some(reply)));
    let answered = slot.clone();
    let answer = Box::new(move |result: Result<Value, Error>| {
        if let Some(reply) = answered.lock().unwrap_or_else(|e| e.into_inner()).take() {
            reply(result.map(|body| convert(&body)));
        }
    });
    if let Err(error) = connection.request(command, arguments, SHORT, answer)
        && let Some(reply) = slot.lock().unwrap_or_else(|e| e.into_inner()).take()
    {
        reply(Err(error));
    }
}

/// The source argument for `path`.
pub(crate) fn source(path: &Path) -> Value {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    json!({"path": path, "name": name})
}

impl Session {
    /// The program's threads.
    pub fn threads(&self, reply: Reply<Vec<Thread>>) {
        let weak = super::weak(self.inner());
        send(
            self.inner(),
            "threads",
            Value::Null,
            move |body| {
                let threads = convert::threads(body);
                if let Some(inner) = weak.upgrade() {
                    inner.lock().threads.clone_from(&threads);
                }
                threads
            },
            reply,
        );
    }

    /// The call stack of `thread`, innermost frame first.
    pub fn stack_trace(&self, thread: ThreadId, reply: Reply<Vec<StackFrame>>) {
        send(
            self.inner(),
            "stackTrace",
            json!({"threadId": thread, "startFrame": 0, "levels": 0}),
            convert::stack_frames,
            reply,
        );
    }

    /// The scopes of `frame`, such as locals and registers.
    pub fn scopes(&self, frame: FrameId, reply: Reply<Vec<Scope>>) {
        send(
            self.inner(),
            "scopes",
            json!({"frameId": frame}),
            convert::scopes,
            reply,
        );
    }

    /// The children of a scope, variable or result; fetched when the user opens it.
    pub fn variables(&self, of: VariablesRef, reply: Reply<Vec<Variable>>) {
        if !of.has_children() {
            return reply(Ok(Vec::new()));
        }
        send(
            self.inner(),
            "variables",
            json!({"variablesReference": of}),
            convert::variables,
            reply,
        );
    }

    /// The value of `expression` in `frame`, or globally without one.
    pub fn evaluate(
        &self,
        expression: &str,
        frame: Option<FrameId>,
        context: EvaluateContext,
        reply: Reply<Evaluation>,
    ) {
        let mut arguments = json!({"expression": expression, "context": context.wire()});
        if let Some(frame) = frame {
            arguments["frameId"] = json!(frame);
        }
        send(self.inner(), "evaluate", arguments, convert::evaluation, reply);
    }

    /// Lets the program run on; `thread` names the thread the user chose, though most adapters continue them all.
    pub fn resume(&self, thread: ThreadId, reply: Reply<()>) {
        self.execute("continue", thread, reply);
    }

    /// Steps over one line of `thread`.
    pub fn next(&self, thread: ThreadId, reply: Reply<()>) {
        self.execute("next", thread, reply);
    }

    /// Steps into the call on the current line of `thread`.
    pub fn step_in(&self, thread: ThreadId, reply: Reply<()>) {
        self.execute("stepIn", thread, reply);
    }

    /// Runs `thread` until its current function returns.
    pub fn step_out(&self, thread: ThreadId, reply: Reply<()>) {
        self.execute("stepOut", thread, reply);
    }

    /// Interrupts `thread`; `Event::Stopped` with `StopReason::Pause` follows.
    pub fn pause(&self, thread: ThreadId, reply: Reply<()>) {
        if !matches!(self.inner().lock().state, State::Running) {
            return reply(Err(Error::NotStopped));
        }
        send(self.inner(), "pause", json!({"threadId": thread}), |_| (), reply);
    }

    /// Restarts the program within the session, if the adapter can (`Capabilities::restart`); otherwise the app stops
    /// the session and starts a new one with the same configuration.
    pub fn restart(&self, reply: Reply<()>) {
        let inner = self.inner();
        if !inner.lock().capabilities.restart {
            return reply(Err(Error::Unsupported));
        }
        let arguments = json!({"arguments": Value::Object(inner.config.arguments.clone())});
        send(inner, "restart", arguments, |_| (), reply);
    }

    /// Replaces the breakpoints of `path` with `breakpoints`; the answer lists what the adapter made of each, in
    /// the same order. Before the adapter is ready they are kept and set with the others, and the answer lists them
    /// unverified.
    pub fn set_breakpoints(&self, path: &Path, breakpoints: Vec<SourceBreakpoint>, reply: Reply<Vec<Breakpoint>>) {
        let (configured, children) = {
            let mut shared = self.inner().lock();
            if breakpoints.is_empty() {
                shared.setup.breakpoints.remove(path);
            } else {
                shared.setup.breakpoints.insert(path.to_path_buf(), breakpoints.clone());
            }
            (shared.configured, shared.children.clone())
        };
        for child in children {
            child.set_breakpoints(path, breakpoints.clone(), Box::new(|_| {}));
        }
        if !configured {
            let pending = breakpoints
                .iter()
                .map(|b| Breakpoint {
                    path: Some(path.to_path_buf()),
                    line: Some(b.line),
                    column: b.column,
                    ..Breakpoint::default()
                })
                .collect();
            return reply(Ok(pending));
        }
        set_breakpoints(self.inner(), path, &breakpoints, reply);
    }

    /// Turns on exactly these exception filters, by `ExceptionFilter::filter`.
    pub fn set_exception_filters(&self, filters: Vec<String>, reply: Reply<()>) {
        let (configured, children) = {
            let mut shared = self.inner().lock();
            shared.setup.exception_filters = Some(filters.clone());
            (shared.configured, shared.children.clone())
        };
        for child in children {
            child.set_exception_filters(filters.clone(), Box::new(|_| {}));
        }
        if !configured {
            return reply(Ok(()));
        }
        send(
            self.inner(),
            "setExceptionBreakpoints",
            json!({"filters": filters}),
            |_| (),
            reply,
        );
    }

    /// Sends a stepping command for a stopped program; on success the session runs again.
    fn execute(&self, command: &'static str, thread: ThreadId, reply: Reply<()>) {
        let inner = self.inner();
        let stops = {
            let shared = inner.lock();
            if !matches!(shared.state, State::Stopped(_)) {
                drop(shared);
                return reply(Err(Error::NotStopped));
            }
            shared.stops
        };
        let weak = super::weak(inner);
        send(
            inner,
            command,
            json!({"threadId": thread}),
            move |body| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let all_threads = body.get("allThreadsContinued").and_then(Value::as_bool).unwrap_or(true);
                {
                    let mut shared = inner.lock();
                    // A stop that arrived before this answer is newer than it.
                    if shared.stops != stops || !matches!(shared.state, State::Stopped(_)) {
                        return;
                    }
                    shared.state = State::Running;
                }
                inner.changed.notify_all();
                inner.emit(Event::Continued {
                    thread: Some(thread),
                    all_threads,
                });
            },
            reply,
        );
    }
}

/// Sends the breakpoints of `path` and reports what the adapter made of them.
pub(crate) fn set_breakpoints(
    inner: &Inner,
    path: &Path,
    breakpoints: &[SourceBreakpoint],
    reply: Reply<Vec<Breakpoint>>,
) {
    let id_path = path.to_path_buf();
    let sink = inner.sink.clone();
    let id = inner.id;
    let lines: Vec<u32> = breakpoints.iter().map(|b| b.line).collect();
    send(
        inner,
        "setBreakpoints",
        json!({"source": source(path), "breakpoints": breakpoints, "lines": lines}),
        move |body| {
            let mut set = convert::breakpoints(body);
            for breakpoint in &mut set {
                breakpoint.path.get_or_insert_with(|| id_path.clone());
            }
            let event = Event::BreakpointsSet {
                path: id_path,
                breakpoints: set.clone(),
            };
            crate::connection::contained(|| sink(id, event));
            set
        },
        reply,
    );
}
