//! The end of a session by who owns what (Review C7): the adapter always belongs to the app, a launched program to
//! the session, an attached program to nobody here, and a program in a terminal tab to that terminal.

use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use ide_process::{Ending, ProcessId};
use serde_json::{Value, json};

use super::{Event, Inner, start};
use crate::error::Error;
use crate::ownership;
use crate::types::{End, EndReason, Request, StartMethod, State};

/// How long each step of stopping may take before the next, harder one: `terminate`, `disconnect`, the signals.
pub(crate) const GRACE: Duration = Duration::from_secs(2);

/// Every session that has not ended, for quitting the app.
static LIVE: Mutex<Vec<Weak<Inner>>> = Mutex::new(Vec::new());

pub(super) fn remember(inner: &Arc<Inner>) {
    let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    live.retain(|weak| weak.strong_count() > 0);
    live.push(Arc::downgrade(inner));
}

/// Stops every session that runs, as `Session::stop` does, and waits up to `timeout` for them to end: launched
/// programs end, attached ones are detached. For quitting the app, before the supervisor's own shutdown.
pub fn stop_all(timeout: Duration) {
    let live: Vec<Arc<Inner>> = LIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for inner in &live {
        begin(inner);
    }
    let deadline = Instant::now() + timeout;
    for inner in &live {
        let mut shared = inner.lock();
        while !matches!(shared.state, State::Ended(_)) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            shared = inner
                .changed
                .wait_timeout(shared, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// Starts stopping in the background, once.
pub(super) fn begin(inner: &Arc<Inner>) {
    {
        let mut shared = inner.lock();
        if shared.stopping || matches!(shared.state, State::Ended(_)) {
            return;
        }
        shared.stopping = true;
        shared.intended.get_or_insert(EndReason::Stopped);
        shared.state = State::Stopping;
    }
    inner.changed.notify_all();
    inner.emit(Event::Stopping);
    let inner = inner.clone();
    let _ = std::thread::Builder::new()
        .name("dap-stop".into())
        .spawn(move || end(&inner));
}

/// Stops the children, says goodbye to the adapter, and ends its process.
fn end(inner: &Arc<Inner>) {
    let children = inner.lock().children.clone();
    for child in &children {
        child.stop();
    }
    for child in &children {
        child.wait_until_ended(GRACE * 3);
    }
    goodbye(inner);
    finish_adapter(inner, EndReason::Stopped);
}

/// Asks the adapter to let go of the program as its owner decides; blocks for up to twice `GRACE`, so never on the
/// thread that reads the adapter.
pub(crate) fn goodbye(inner: &Arc<Inner>) {
    let (connection, capabilities, terminated) = {
        let mut shared = inner.lock();
        if std::mem::replace(&mut shared.said_goodbye, true) {
            return;
        }
        (
            shared.connection.clone(),
            shared.capabilities.clone(),
            shared.terminated,
        )
    };
    let Some(connection) = connection.filter(|c| c.is_open()) else {
        return;
    };
    let ours = inner.config.request == Request::Launch;
    if ours && capabilities.terminate && !terminated {
        let _ = ask(&connection, "terminate", json!({"restart": false}));
        wait_terminated(inner, GRACE);
    }
    // An attached program never gets a signal from the app: the adapter only detaches (Review C7).
    let _ = ask(
        &connection,
        "disconnect",
        json!({"restart": false, "terminateDebuggee": ours}),
    );
}

/// Sends `command` and waits up to `GRACE` for its answer.
fn ask(connection: &crate::connection::Connection, command: &str, arguments: Value) -> Result<Value, Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    connection.request(
        command,
        arguments,
        GRACE,
        Box::new(move |answer| {
            let _ = tx.send(answer);
        }),
    )?;
    rx.recv_timeout(GRACE + Duration::from_millis(100))
        .unwrap_or(Err(Error::Timeout))
}

fn wait_terminated(inner: &Inner, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    let mut shared = inner.lock();
    while !shared.terminated && !matches!(shared.state, State::Ended(_)) {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        shared = inner
            .changed
            .wait_timeout(shared, deadline - now)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

/// Ends the adapter process through the supervisor, whose end then finishes the session; a session without a
/// process of its own finishes now.
fn finish_adapter(inner: &Arc<Inner>, reason: EndReason) {
    let adapter = inner.lock().adapter;
    match adapter {
        Some(id) => {
            // The supervisor's graceful step is `goodbye`, which has run; next come the signals to the group.
            inner.supervisor.stop(id, GRACE);
            if inner.supervisor.wait(id, GRACE * 4).is_none() {
                finish(inner, reason);
            }
        }
        None => finish(inner, reason),
    }
}

/// Watches the adapter process: its end ends the session, as a crash when nobody asked for it.
pub(super) fn watch_adapter(inner: &Arc<Inner>, process: ProcessId) {
    let weak = Arc::downgrade(inner);
    let graceful = weak.clone();
    inner.supervisor.on_stop(process, move || {
        if let Some(inner) = graceful.upgrade() {
            {
                let mut shared = inner.lock();
                if !matches!(shared.state, State::Ended(_)) {
                    shared.stopping = true;
                    shared.intended.get_or_insert(EndReason::Stopped);
                }
            }
            goodbye(&inner);
        }
    });
    let supervisor = inner.supervisor.clone();
    let _ = std::thread::Builder::new()
        .name(format!("dap-wait-{}", inner.id.get()))
        .spawn(move || {
            let ending = loop {
                if let Some(ending) = supervisor.wait(process, Duration::from_secs(3600)) {
                    break ending;
                }
                if weak.strong_count() == 0 {
                    return;
                }
            };
            if let Some(inner) = weak.upgrade() {
                adapter_ended(&inner, ending);
            }
        });
}

fn adapter_ended(inner: &Arc<Inner>, ending: Ending) {
    let reason = {
        let shared = inner.lock();
        if let Some(intended) = &shared.intended {
            intended.clone()
        } else if shared.terminated || (shared.exit_code.is_some() && ending == Ending::Code(0)) {
            EndReason::Finished
        } else {
            EndReason::AdapterCrashed {
                ending: Some(ending),
                stderr: shared.stderr.lock().unwrap_or_else(|e| e.into_inner()).text(),
            }
        }
    };
    finish(inner, reason);
}

/// The connection broke or ended; for an adapter process its end decides, otherwise this is the end.
pub(super) fn connection_closed(inner: &Arc<Inner>) {
    let (adapter, reason) = {
        let shared = inner.lock();
        let reason = if let Some(intended) = &shared.intended {
            intended.clone()
        } else if shared.terminated {
            EndReason::Finished
        } else {
            EndReason::AdapterCrashed {
                ending: None,
                stderr: String::new(),
            }
        };
        (shared.adapter, reason)
    };
    match adapter {
        // An adapter that closed its output without ending is ended now; its end finishes the session.
        Some(id) => {
            let inner = inner.clone();
            let _ = std::thread::Builder::new().name("dap-closed".into()).spawn(move || {
                if inner.supervisor.wait(id, GRACE).is_none() {
                    inner.supervisor.stop(id, GRACE);
                }
            });
        }
        None => finish(inner, reason),
    }
}

/// The adapter says the session is over: it is closed as the user would stop it, without asking the program to end.
pub(super) fn terminated(inner: &Arc<Inner>) {
    {
        let mut shared = inner.lock();
        shared.terminated = true;
        if shared.stopping || matches!(shared.state, State::Ended(_)) {
            drop(shared);
            inner.changed.notify_all();
            return;
        }
        shared.stopping = true;
        shared.intended.get_or_insert(EndReason::Finished);
    }
    inner.changed.notify_all();
    let inner = inner.clone();
    let _ = std::thread::Builder::new()
        .name("dap-terminated".into())
        .spawn(move || {
            let children = inner.lock().children.clone();
            for child in &children {
                child.stop_and_wait(GRACE * 3);
            }
            goodbye(&inner);
            finish_adapter(&inner, EndReason::Finished);
        });
}

/// Initializing, launching or attaching failed: the session ends with the adapter's words and its process goes.
pub(super) fn fail(inner: &Arc<Inner>, error: &Error) {
    // A connection that closed fails what waits on it; its end, or the adapter's, says why the session ended.
    if *error == Error::Ended {
        return;
    }
    let adapter = {
        let mut shared = inner.lock();
        if matches!(shared.state, State::Ended(_)) {
            return;
        }
        shared.stopping = true;
        shared.adapter
    };
    finish(inner, start::failed_reason(error));
    if let Some(id) = adapter {
        inner.supervisor.stop(id, GRACE);
    }
}

/// Ends the session once with `reason`; a launched program that is still there goes with its group.
pub(super) fn finish(inner: &Arc<Inner>, reason: EndReason) {
    let (end, connection, program) = {
        let mut shared = inner.lock();
        if matches!(shared.state, State::Ended(_)) {
            return;
        }
        let end = End {
            reason,
            exit_code: shared.exit_code,
        };
        shared.state = State::Ended(end.clone());
        let owned = inner.config.request == Request::Launch
            && !shared.in_terminal
            && shared
                .program
                .as_ref()
                .is_some_and(|p| p.start.is_none_or(|start| start == StartMethod::Launch));
        let program = owned.then(|| shared.program.as_ref().and_then(|p| p.pid)).flatten();
        (end, shared.connection.take(), program)
    };
    inner.changed.notify_all();
    if let Some(connection) = connection {
        connection.close();
    }
    if let Some(pid) = program {
        ownership::end_program(pid, GRACE);
    }
    inner.emit(Event::Ended(end));
}
