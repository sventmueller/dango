//! Reaching the adapter and the start of the protocol: `initialize`, then `launch` or `attach`, and once the adapter
//! sends `initialized`, the breakpoints, the exception filters and `configurationDone`.

use std::fs::File;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ide_process::{Kind, ProcessId};
use serde_json::{Value, json};

use super::{Event, Inner, LONG, Reach, SHORT, handler, requests, stop};
use crate::adapter::{Adapter, Transport};
use crate::connection::Connection;
use crate::error::Error;
#[cfg(any(test, feature = "testing"))]
use crate::fake;
use crate::types::{Capabilities, EndReason, Request, State};

/// How long an adapter that listens on a port may take to take connections.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Starts or reaches the adapter and connects to it.
pub(super) fn connect(inner: &Arc<Inner>, adapter: Adapter) -> io::Result<()> {
    let record = match &inner.record {
        Some(path) => Some(File::create(path)?),
        None => None,
    };
    let handler = Arc::new(handler::Events::new(inner));
    let name = inner.id.get().to_string();
    let connection = match adapter.transport {
        Transport::Process {
            program,
            args,
            env,
            cwd,
            port,
        } => {
            let (process, reader, writer) = spawn(inner, &program, &args, &env, &cwd, port)?;
            {
                let mut shared = inner.lock();
                shared.adapter = Some(process);
                if let Some(port) = port {
                    shared.reach = Reach::Port(port);
                }
            }
            stop::watch_adapter(inner, process);
            Connection::start(reader, writer, handler, &name, record)?
        }
        Transport::Streams(reader, writer) => Connection::start(reader, writer, handler, &name, record)?,
        #[cfg(any(test, feature = "testing"))]
        Transport::Fake(script) => {
            let (reader, writer) = fake::start(*script)?;
            Connection::start(reader, writer, handler, &name, record)?
        }
    };
    inner.lock().connection = Some(connection);
    Ok(())
}

type Streams = (ProcessId, Box<dyn Read + Send>, Box<dyn Write + Send>);

/// Starts the adapter process through the supervisor and returns the streams it speaks on.
fn spawn(
    inner: &Arc<Inner>,
    program: &Path,
    args: &[String],
    env: &[(String, String)],
    cwd: &Path,
    port: Option<u16>,
) -> io::Result<Streams> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let spawned = inner
        .supervisor
        .spawn(command, Kind::DebugAdapter, inner.label.clone())?;
    let id = spawned.id;
    let stderr_tail = inner.lock().stderr.clone();
    if let Some(stderr) = spawned.stderr {
        let _ = ide_process::tail::collect(stderr, stderr_tail.clone());
    }
    let Some(port) = port else {
        let (Some(stdin), Some(stdout)) = (spawned.stdin, spawned.stdout) else {
            inner.supervisor.stop(id, Duration::from_millis(500));
            return Err(io::Error::other("the adapter's pipes are missing"));
        };
        return Ok((id, Box::new(stdout), Box::new(stdin)));
    };
    // An adapter that listens prints its address on standard output; what it prints is only kept for the crash
    // message.
    if let Some(stdout) = spawned.stdout {
        let _ = ide_process::tail::collect(stdout, stderr_tail);
    }
    drop(spawned.stdin);
    match dial(port, || inner.supervisor.wait(id, Duration::ZERO).is_none()) {
        Ok(stream) => {
            let reader = stream.try_clone()?;
            Ok((id, Box::new(reader), Box::new(stream)))
        }
        Err(error) => {
            inner.supervisor.stop(id, Duration::from_millis(500));
            Err(error)
        }
    }
}

/// Connects to `port` on the loopback interface, trying again while the adapter starts and `alive` holds.
pub(crate) fn dial(port: u16, alive: impl Fn() -> bool) -> io::Result<TcpStream> {
    let deadline = Instant::now() + LISTEN_TIMEOUT;
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                return Ok(stream);
            }
            Err(error) if Instant::now() >= deadline || !alive() => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// The arguments of `initialize`, as a client that runs programs in its own terminal tabs and takes child sessions.
fn initialize_arguments(inner: &Inner) -> Value {
    json!({
        "clientID": ide_product::COMMAND,
        "clientName": ide_product::NAME,
        "adapterID": inner.kind.adapter_id(),
        "locale": "en",
        "linesStartAt1": true,
        "columnsStartAt1": true,
        "pathFormat": "path",
        "supportsVariableType": true,
        "supportsVariablePaging": false,
        "supportsRunInTerminalRequest": true,
        "supportsStartDebuggingRequest": true,
        "supportsInvalidatedEvent": true,
        "supportsProgressReporting": false,
        "supportsMemoryReferences": false,
        "supportsArgsCanBeInterpretedByShell": false,
    })
}

/// Sends `initialize`, then `launch` or `attach`; the configuration follows the adapter's `initialized` event.
pub(super) fn initialize(inner: &Arc<Inner>) {
    let Ok(connection) = inner.connection() else {
        return;
    };
    let weak = super::weak(inner);
    let answer = Box::new(move |result: Result<Value, Error>| {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(body) => {
                let capabilities = Capabilities::from_json(&body);
                inner.lock().capabilities = capabilities.clone();
                inner.emit(Event::Initialized(capabilities));
                begin(&inner);
            }
            Err(error) => stop::fail(&inner, &error),
        }
    });
    if let Err(error) = connection.request("initialize", initialize_arguments(inner), SHORT, answer) {
        stop::fail(inner, &error);
    }
}

/// Sends `launch` or `attach` with the configuration's arguments.
fn begin(inner: &Arc<Inner>) {
    let Ok(connection) = inner.connection() else {
        return;
    };
    let command = match inner.config.request {
        Request::Launch => "launch",
        Request::Attach => "attach",
    };
    let mut arguments = inner.config.arguments.clone();
    arguments.insert("type".into(), json!(inner.config.adapter_type()));
    arguments.insert("request".into(), json!(command));
    arguments.insert("name".into(), json!(inner.config.name));
    let weak = super::weak(inner);
    let answer = Box::new(move |result: Result<Value, Error>| {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(_) => {
                inner.lock().course.launched();
                running_if_ready(&inner);
            }
            Err(error) => stop::fail(&inner, &error),
        }
    });
    if let Err(error) = connection.request(command, Value::Object(arguments), LONG, answer) {
        stop::fail(inner, &error);
    }
}

/// Sets the breakpoints and exception filters, then `configurationDone`; called on the adapter's `initialized`.
pub(super) fn configure(inner: &Arc<Inner>) {
    let (setup, capabilities) = {
        let shared = inner.lock();
        (shared.setup.clone(), shared.capabilities.clone())
    };
    for (path, breakpoints) in &setup.breakpoints {
        requests::set_breakpoints(inner, path, breakpoints, Box::new(|_| {}));
    }
    let filters = setup.exception_filters.unwrap_or_else(|| {
        capabilities
            .exception_filters
            .iter()
            .filter(|f| f.default)
            .map(|f| f.filter.clone())
            .collect()
    });
    let (filters, functions) = crate::filters::split(&filters, &capabilities);
    if !capabilities.exception_filters.is_empty() || !filters.is_empty() {
        requests::send(
            inner,
            "setExceptionBreakpoints",
            json!({"filters": filters}),
            |_| (),
            Box::new(|_| {}),
        );
    }
    if !functions.is_empty() {
        requests::set_functions(inner, &functions);
    }
    if !capabilities.configuration_done {
        configured(inner);
        return;
    }
    let weak = super::weak(inner);
    requests::send(
        inner,
        "configurationDone",
        Value::Null,
        |_| (),
        Box::new(move |result| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(()) => configured(&inner),
                Err(error) => stop::fail(&inner, &error),
            }
        }),
    );
}

fn configured(inner: &Arc<Inner>) {
    inner.lock().course.configured();
    running_if_ready(inner);
}

/// The session runs once the program was launched or attached and the breakpoints are set, unless it stopped
/// already, such as at its entry.
fn running_if_ready(inner: &Arc<Inner>) {
    {
        let mut shared = inner.lock();
        if !shared.course.is_ready() || !matches!(shared.state, State::Starting) {
            return;
        }
        shared.state = State::Running;
    }
    inner.changed.notify_all();
    inner.emit(Event::Running);
}

/// Whether the session failed before it ran, for the message.
pub(super) fn failed_reason(error: &Error) -> EndReason {
    match error {
        Error::Adapter { message, .. } if !message.is_empty() => EndReason::Failed(message.clone()),
        other => EndReason::Failed(other.to_string()),
    }
}
