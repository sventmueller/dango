//! Opening a session, in the order the server requires: the version check, `initialize`, `initialized`, then the
//! account, the thread (`thread/start` or `thread/resume`) and the models.

use std::sync::Arc;
use std::time::Duration;

use ide_thread::{Status, Update};
use serde_json::{Value, json};

use super::{After, Event, SessionError, Shared, now, reply};
use crate::mapping::{self, Mapper};
use crate::protocol::{method, text};

/// Runs the handshake; what goes wrong is reported, and an unsupported release also ends the process.
pub(super) fn open(shared: &Arc<Shared>) {
    match handshake(shared) {
        Ok(()) => {}
        Err(error @ SessionError::Unsupported { .. }) => {
            shared.fail(error);
            shared.stopping.store(true, std::sync::atomic::Ordering::SeqCst);
            shared.supervisor.stop(shared.process, Duration::from_millis(500));
        }
        Err(SessionError::Closed) if shared.ended.load(std::sync::atomic::Ordering::SeqCst) => {}
        Err(error) => shared.fail(error),
    }
}

/// The parameters of `initialize` for a server of its own: the client's name, without the experimental API, whose
/// methods and fields this crate does not use there.
pub(crate) fn initialize_params() -> Value {
    initialize_with(false)
}

/// The parameters of `initialize`, with the experimental API when `experimental`: the shared server's Remote Control
/// switch is only there.
fn initialize_with(experimental: bool) -> Value {
    json!({
        "clientInfo": {"name": ide_product::NAME, "title": ide_product::NAME, "version": env!("CARGO_PKG_VERSION")},
        "capabilities": {"experimentalApi": experimental}
    })
}

fn handshake(shared: &Arc<Shared>) -> Result<(), SessionError> {
    let launch = &shared.launch;
    let version = match launch.version {
        Some(known) => crate::version::check(Some(known))?,
        None => crate::version::probe(&shared.supervisor, &launch.program, &launch.env)?,
    };
    shared.state().caps.version = Some(version);
    let result = shared
        .call(method::INITIALIZE, initialize_with(launch.shared), None, reply::raw)
        .wait()?;
    shared.state().caps.learn_initialize(&result);
    shared.emit(Event::Capabilities);
    shared.notify(method::INITIALIZED, Value::Null)?;
    let account = shared
        .call(method::ACCOUNT_READ, json!({"refreshToken": false}), None, reply::raw)
        .wait()?;
    let signed_in = {
        let mut state = shared.state();
        state.caps.learn_account(&account);
        state.caps.signed_in()
    };
    let subscription = mapping::usage_subscription(&shared.state().caps);
    shared.apply(vec![subscription]);
    if !signed_in {
        shared.fail(SessionError::SignIn);
    }
    open_thread(shared)?;
    let after: After = Box::new(|result, shared| {
        if let Ok(models) = result {
            shared.state().caps.learn_models(models);
            shared.emit(Event::Capabilities);
        }
    });
    drop(shared.call(method::MODEL_LIST, json!({}), Some(after), reply::unit));
    Ok(())
}

fn open_thread(shared: &Arc<Shared>) -> Result<(), SessionError> {
    let launch = &shared.launch;
    let resuming = launch.resume.is_some();
    let request = if resuming {
        method::THREAD_RESUME
    } else {
        method::THREAD_START
    };
    let result = shared.call(request, launch.open_params(), None, reply::raw).wait()?;
    let thread = result.get("thread").unwrap_or(&Value::Null);
    let id = text(thread, "id")
        .ok_or_else(|| SessionError::Malformed(format!("{request} named no thread")))?
        .to_string();
    let updates = {
        let mut state = shared.state();
        let state = &mut *state;
        state.caps.learn_thread(&result);
        state.mapper.set_thread(&id);
        let mut updates = Vec::new();
        if resuming {
            updates.extend(state.mapper.history(thread, now()));
        }
        updates.extend(header(&mut state.mapper, &state.caps, &id));
        state.thread_id = Some(id);
        updates
    };
    shared.apply(updates);
    shared.emit(Event::Capabilities);
    Ok(())
}

/// The thread's head as the opened thread sets it.
fn header(mapper: &mut Mapper, caps: &crate::Capabilities, id: &str) -> Vec<Update> {
    let mut updates = mapper.settle(now());
    updates.push(Update::Session(id.to_string()));
    if let Some(model) = &caps.model {
        updates.push(Update::Model(model.clone()));
    }
    updates.push(Update::Effort(caps.effort.clone()));
    if let Some(mode) = caps.mode() {
        updates.push(Update::Mode(mode.to_string()));
    }
    updates.push(Update::Status(Status::Idle));
    updates
}
