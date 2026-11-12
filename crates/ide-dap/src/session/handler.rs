//! What the adapter sends on its own: events, which become the session's state and `Event`s, and its requests to run
//! a program in a terminal or to start a child session.

use std::path::PathBuf;
use std::sync::{Arc, Weak};

use serde_json::{Map, Value};

use super::{Event, Inner, Reach, Session, TerminalAnswer, start, stop};
use crate::adapter::Adapter;
use crate::connection::{Handler, Responder};
use crate::convert;
use crate::launch::LaunchConfig;
use crate::types::{Request, State, TerminalRequest, ThreadId};

pub(super) struct Events {
    inner: Weak<Inner>,
}

impl Events {
    pub(super) fn new(inner: &Arc<Inner>) -> Events {
        Events {
            inner: Arc::downgrade(inner),
        }
    }
}

impl Handler for Events {
    fn event(&self, event: &str, body: Value) {
        if let Some(inner) = self.inner.upgrade() {
            on_event(&inner, event, &body);
        }
    }

    fn request(&self, command: &str, arguments: Value, responder: Responder) {
        let Some(inner) = self.inner.upgrade() else {
            return responder.fail("the session has ended");
        };
        match command {
            "runInTerminal" => run_in_terminal(&inner, &arguments, responder),
            "startDebugging" => start_child(&inner, &arguments, responder),
            _ => responder.fail("not supported"),
        }
    }

    fn closed(&self) {
        if let Some(inner) = self.inner.upgrade() {
            stop::connection_closed(&inner);
        }
    }
}

fn thread_of(body: &Value) -> Option<ThreadId> {
    body.get("threadId").and_then(Value::as_i64).map(ThreadId)
}

fn on_event(inner: &Arc<Inner>, event: &str, body: &Value) {
    match event {
        "initialized" => start::configure(inner),
        "capabilities" => {
            let capabilities = {
                let mut shared = inner.lock();
                if let Some(changed) = body.get("capabilities") {
                    shared.capabilities.update(changed);
                }
                shared.capabilities.clone()
            };
            inner.emit(Event::Capabilities(capabilities));
        }
        "stopped" => {
            let stop = convert::stop(body);
            {
                let mut shared = inner.lock();
                if matches!(shared.state, State::Ended(_) | State::Stopping) {
                    return;
                }
                shared.stops += 1;
                shared.state = State::Stopped(stop.clone());
            }
            inner.changed.notify_all();
            inner.emit(Event::Stopped(stop));
        }
        "continued" => continued(inner, body),
        "thread" => match body.get("reason").and_then(Value::as_str) {
            Some("started") => thread_of(body)
                .into_iter()
                .for_each(|t| inner.emit(Event::ThreadStarted(t))),
            Some("exited") => thread_of(body)
                .into_iter()
                .for_each(|t| inner.emit(Event::ThreadExited(t))),
            _ => {}
        },
        "output" => {
            if let Some(output) = convert::output(body) {
                inner.emit(Event::Output(output));
            }
        }
        "breakpoint" => {
            if let Some(breakpoint) = body.get("breakpoint") {
                inner.emit(Event::Breakpoint(convert::breakpoint(breakpoint)));
            }
        }
        "process" => {
            let program = convert::program(body);
            inner.lock().program = Some(program.clone());
            inner.emit(Event::Program(program));
        }
        "exited" => {
            if let Some(code) = body.get("exitCode").and_then(Value::as_i64) {
                inner.lock().exit_code = Some(code);
                inner.emit(Event::Exited(code));
            }
        }
        "terminated" => stop::terminated(inner),
        "invalidated" => inner.emit(Event::Invalidated),
        _ => {}
    }
}

fn continued(inner: &Arc<Inner>, body: &Value) {
    {
        let mut shared = inner.lock();
        if !matches!(shared.state, State::Stopped(_)) {
            return;
        }
        shared.state = State::Running;
    }
    inner.changed.notify_all();
    inner.emit(Event::Continued {
        thread: thread_of(body),
        all_threads: body.get("allThreadsContinued").and_then(Value::as_bool).unwrap_or(true),
    });
}

fn run_in_terminal(inner: &Arc<Inner>, arguments: &Value, responder: Responder) {
    let args: Vec<String> = arguments
        .get("args")
        .and_then(Value::as_array)
        .map(|args| args.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    if args.is_empty() {
        return responder.fail("runInTerminal without a program");
    }
    let env = arguments
        .get("env")
        .and_then(Value::as_object)
        .map(|env| {
            env.iter()
                .map(|(key, value)| (key.clone(), value.as_str().map(str::to_string)))
                .collect()
        })
        .unwrap_or_default();
    let request = TerminalRequest {
        title: arguments.get("title").and_then(Value::as_str).map(str::to_string),
        cwd: PathBuf::from(arguments.get("cwd").and_then(Value::as_str).unwrap_or("/")),
        args,
        env,
        external: arguments.get("kind").and_then(Value::as_str) == Some("external"),
    };
    inner.lock().in_terminal = true;
    inner.emit(Event::RunInTerminal(request, TerminalAnswer { responder }));
}

/// Starts the session the adapter asks for beneath this one, on a connection of its own to the same adapter.
fn start_child(inner: &Arc<Inner>, arguments: &Value, responder: Responder) {
    let port = match inner.lock().reach {
        Reach::Port(port) => port,
        Reach::Single => return responder.fail("child sessions need an adapter that takes several connections"),
    };
    let request = match arguments.get("request").and_then(Value::as_str) {
        Some("attach") => Request::Attach,
        _ => Request::Launch,
    };
    let configuration: Map<String, Value> = arguments
        .get("configuration")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let name = configuration
        .get("name")
        .and_then(Value::as_str)
        .map_or_else(|| inner.config.name.clone(), str::to_string);
    let config = LaunchConfig {
        name,
        kind: inner.kind,
        // A child's program is the parent's: it belongs to the session the user started (Review C7).
        request: inner.config.request.max_ownership(request),
        arguments: configuration,
        process_pick: false,
    };
    let alive = {
        let supervisor = inner.supervisor.clone();
        let adapter = inner.lock().adapter;
        move || adapter.is_none_or(|id| supervisor.wait(id, std::time::Duration::ZERO).is_none())
    };
    let stream = match start::dial(port, alive).and_then(|s| Ok((s.try_clone()?, s))) {
        Ok(streams) => streams,
        Err(error) => return responder.fail(&error.to_string()),
    };
    let adapter = Adapter::streams(inner.kind, &inner.label, stream.0, stream.1);
    let setup = inner.lock().setup.clone();
    match Session::begin(&inner.supervisor, adapter, config, setup, inner.sink.clone()) {
        Ok(child) => {
            child.inner().lock().reach = Reach::Port(port);
            responder.ok(Value::Null);
            inner.lock().children.push(child.clone());
            inner.emit(Event::Child(child));
        }
        Err(error) => responder.fail(&error.to_string()),
    }
}

impl Request {
    /// The ownership a child session gets: whatever the adapter asks, a program of a launched session belongs to it.
    fn max_ownership(self, child: Request) -> Request {
        match self {
            Request::Launch => Request::Launch,
            Request::Attach => child,
        }
    }
}
