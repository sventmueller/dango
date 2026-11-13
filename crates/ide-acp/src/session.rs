//! A running ACP agent: the process the supervisor started, the protocol connection over its pipes, and the thread
//! the connection fills.
//!
//! The connection runs on a thread of its own; requests go out from short-lived threads that wait for their answer,
//! so neither the UI nor the connection's dispatch ever waits for the agent.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema as acp;
use agent_client_protocol::{Agent, Builder, ByteStreams, Client, ConnectionTo, JsonRpcRequest, Responder};
use futures::channel::oneshot;
use ide_process::tail::{self, Tail};
use ide_process::{Kind, ProcessId, Supervisor};
use ide_thread::{Change, Decision, ErrorKind, RequestState, Status, Thread, Update, UserMessage};

use crate::capabilities::Capabilities;
use crate::launch::Launch;
use crate::mapping::Mapper;
use crate::prompt;

/// What the stderr tail keeps.
const STDERR_LIMIT: usize = 64 * 1024;

/// What a session tells the UI, on one of the session's threads.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The thread changed; read it with `Session::snapshot`.
    Changed(Vec<Change>),
    /// What the agent or its session offers changed.
    Capabilities,
    /// Something keeps the session from working.
    Failed(SessionError),
    /// The process ended; its last error output is attached.
    Ended {
        /// The end of what the agent wrote to stderr.
        stderr: String,
    },
}

/// Why a session cannot do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The program is not there.
    NotFound(PathBuf),
    /// The process did not start.
    Start(String),
    /// The agent wants a sign-in before it opens a session; the methods are in the capabilities.
    SignIn,
    /// The agent speaks a protocol version this build does not.
    Version(u16),
    /// The agent answered a request with an error.
    Agent(String),
    /// The session has no live connection.
    Closed,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotFound(path) => write!(f, "{} is not there", path.display()),
            SessionError::Start(why) => write!(f, "the agent did not start: {why}"),
            SessionError::SignIn => write!(f, "the agent asks for a sign-in"),
            SessionError::Version(v) => write!(f, "the agent speaks protocol version {v}"),
            SessionError::Agent(why) => write!(f, "{why}"),
            SessionError::Closed => write!(f, "the session has ended"),
        }
    }
}

impl std::error::Error for SessionError {}

/// The user's answer to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The option at this place in the request's list.
    Option(usize),
    /// No option; the agent hears that the request was cancelled.
    Dismiss,
}

struct Ask {
    responder: Responder<acp::RequestPermissionResponse>,
    options: Vec<acp::PermissionOption>,
    /// Whether it is about a git command that destroys work, for which no rule may be stored (E27).
    destructive: bool,
}

/// The option `choice` stands for; for a destructive git command, allowing always counts as allowing once, so that
/// the agent stores no rule for it.
fn picked(options: &[acp::PermissionOption], choice: Choice, destructive: bool) -> Option<&acp::PermissionOption> {
    let Choice::Option(n) = choice else {
        return None;
    };
    match options.get(n) {
        Some(option) if destructive && option.kind == acp::PermissionOptionKind::AllowAlways => {
            options.iter().find(|o| o.kind == acp::PermissionOptionKind::AllowOnce)
        }
        other => other,
    }
}

struct State {
    mapper: Mapper,
    thread: Thread,
    caps: Capabilities,
    session: Option<acp::SessionId>,
    asks: HashMap<String, Ask>,
}

type Sink = Box<dyn Fn(Event) + Send + Sync>;

struct Shared {
    supervisor: Supervisor,
    process: ProcessId,
    launch: Launch,
    state: Mutex<State>,
    sink: Sink,
    connection: Mutex<Option<ConnectionTo<Agent>>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    stderr: Arc<Mutex<Tail>>,
    ended: AtomicBool,
    stopping: AtomicBool,
    next_ask: AtomicU64,
}

/// A running ACP session; clones share it. It runs until `stop` or the supervisor ends it.
#[derive(Clone)]
pub struct Session {
    shared: Arc<Shared>,
}

fn now() -> SystemTime {
    SystemTime::now()
}

/// The agent's name for `value` on the wire, such as `end_turn`.
fn wire_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn connection(&self) -> Option<ConnectionTo<Agent>> {
        self.connection.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn apply(&self, updates: Vec<Update>) {
        if updates.is_empty() {
            return;
        }
        let changes = self.state().thread.apply_all(updates);
        if !changes.is_empty() {
            (self.sink)(Event::Changed(changes));
        }
    }

    fn on_update(&self, notification: acp::SessionNotification) {
        let (updates, learned) = {
            let mut state = self.state();
            let State { mapper, caps, .. } = &mut *state;
            mapper.update(&notification.update, caps, now())
        };
        self.apply(updates);
        if learned {
            (self.sink)(Event::Capabilities);
        }
    }

    fn on_permission(
        &self,
        request: acp::RequestPermissionRequest,
        responder: Responder<acp::RequestPermissionResponse>,
    ) {
        if request.options.is_empty() {
            // Nothing to choose from means nothing the user could allow; the request is refused (E21).
            let _ = responder.respond(acp::RequestPermissionResponse::new(
                acp::RequestPermissionOutcome::Cancelled,
            ));
            return;
        }
        let id = format!("p{}", self.next_ask.fetch_add(1, Ordering::SeqCst) + 1);
        let updates = {
            let mut state = self.state();
            let updates = state
                .mapper
                .permission(&id, &request.tool_call, &request.options, now());
            let destructive = state.mapper.is_destructive(&request.tool_call);
            state.asks.insert(
                id,
                Ask {
                    responder,
                    options: request.options,
                    destructive,
                },
            );
            updates
        };
        self.apply(updates);
    }

    /// Answers every open permission request as cancelled, as the protocol asks when a turn is cancelled.
    fn withdraw_asks(&self) {
        let asks: Vec<(String, Ask)> = self.state().asks.drain().collect();
        let mut updates = Vec::new();
        for (id, ask) in asks {
            let _ = ask.responder.respond(acp::RequestPermissionResponse::new(
                acp::RequestPermissionOutcome::Cancelled,
            ));
            updates.push(Update::Resolve {
                key: format!("permission-{id}"),
                state: RequestState::Withdrawn,
            });
        }
        self.apply(updates);
    }

    fn fail(&self, error: SessionError) {
        (self.sink)(Event::Failed(error));
    }

    /// The connection ended: the process is gone or was stopped.
    fn finish(&self, result: Result<(), agent_client_protocol::Error>) {
        if self.ended.swap(true, Ordering::SeqCst) {
            return;
        }
        *self.connection.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.withdraw_asks();
        let stderr = self.stderr.lock().unwrap_or_else(|e| e.into_inner()).text();
        let running = self.state().thread.status() != Status::Idle;
        if !self.stopping.load(Ordering::SeqCst) && (running || result.is_err()) {
            let message = match &result {
                Err(e) if stderr.trim().is_empty() => e.message.clone(),
                _ => stderr
                    .trim()
                    .lines()
                    .rev()
                    .take(20)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            let mut updates = self.state().mapper.error(ErrorKind::Process, message, now());
            updates.push(Update::Status(Status::Idle));
            self.apply(updates);
        }
        self.supervisor.stop(self.process, Duration::from_millis(500));
        (self.sink)(Event::Ended { stderr });
    }
}

/// Sends `request` and waits for the answer on a thread of its own, then hands it to `then` there.
fn ask<Req>(
    connection: ConnectionTo<Agent>,
    request: Req,
    then: impl FnOnce(Result<Req::Response, agent_client_protocol::Error>) + Send + 'static,
) where
    Req: JsonRpcRequest + Send + 'static,
    Req::Response: Send + 'static,
{
    let sent = connection.send_request(request);
    let _ = std::thread::Builder::new()
        .name("acp-request".into())
        .spawn(move || then(futures::executor::block_on(sent.block_task())));
}

impl Session {
    /// Starts the agent as `launch` describes, filling `thread`; the connection opens a session once the agent
    /// answered `initialize`.
    ///
    /// ```no_run
    /// use std::sync::mpsc::channel;
    ///
    /// use ide_acp::{Event, Launch, Session};
    /// use ide_process::Supervisor;
    /// use ide_thread::{Thread, ThreadId};
    ///
    /// let env: Vec<(String, String)> = std::env::vars().collect();
    /// let launch = Launch::new("gemini", "Gemini CLI", "/opt/homebrew/bin/gemini", vec!["--acp".into()], env, "/work");
    /// let thread = Thread::new(ThreadId::from("t1"), "gemini", "/work");
    /// let (tx, rx) = channel();
    /// let session = Session::start(Supervisor::global(), launch, thread, move |event| drop(tx.send(event)))?;
    /// while !matches!(rx.recv()?, Event::Capabilities) {}
    /// assert_eq!(session.snapshot().agent(), "gemini");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn start(
        supervisor: &Supervisor,
        launch: Launch,
        thread: Thread,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Session, SessionError> {
        if !launch.program.is_file() {
            return Err(SessionError::NotFound(launch.program.clone()));
        }
        let spawned = supervisor
            .spawn(launch.command(), Kind::Agent, launch.name.clone())
            .map_err(|e| SessionError::Start(e.to_string()))?;
        let (Some(stdin), Some(stdout), Some(stderr)) = (spawned.stdin, spawned.stdout, spawned.stderr) else {
            supervisor.stop(spawned.id, Duration::from_millis(500));
            return Err(SessionError::Start("the pipes are missing".into()));
        };
        let stderr_tail = Arc::new(Mutex::new(Tail::new(STDERR_LIMIT)));
        let _ = tail::collect(stderr, stderr_tail.clone());
        let (shutdown, closed) = oneshot::channel();
        let shared = Arc::new(Shared {
            supervisor: supervisor.clone(),
            process: spawned.id,
            launch,
            state: Mutex::new(State {
                mapper: Mapper::new(),
                thread,
                caps: Capabilities::default(),
                session: None,
                asks: HashMap::new(),
            }),
            sink: Box::new(sink),
            connection: Mutex::new(None),
            shutdown: Mutex::new(Some(shutdown)),
            stderr: stderr_tail,
            ended: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            next_ask: AtomicU64::new(0),
        });
        let weak = Arc::downgrade(&shared);
        supervisor.on_stop(spawned.id, move || {
            if let Some(shared) = weak.upgrade() {
                shared.stopping.store(true, Ordering::SeqCst);
                if let Some(shutdown) = shared.shutdown.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    let _ = shutdown.send(());
                }
            }
        });
        let runner = shared.clone();
        std::thread::Builder::new()
            .name(format!("acp-{}", runner.launch.agent))
            .spawn(move || connect(runner, stdin, stdout, closed))
            .map_err(|e| SessionError::Start(e.to_string()))?;
        Ok(Session { shared })
    }

    /// The thread as it is now.
    pub fn snapshot(&self) -> Thread {
        self.shared.state().thread.clone()
    }

    /// Reads the thread without copying it.
    pub fn with_thread<R>(&self, read: impl FnOnce(&Thread) -> R) -> R {
        read(&self.shared.state().thread)
    }

    /// What the agent and its session offer.
    pub fn capabilities(&self) -> Capabilities {
        self.shared.state().caps.clone()
    }

    /// The agent's id for the session, once it opened one.
    pub fn session_id(&self) -> Option<String> {
        self.shared.state().session.as_ref().map(|s| s.0.to_string())
    }

    /// Whether a session is open and takes prompts.
    pub fn is_ready(&self) -> bool {
        !self.is_ended() && self.shared.state().session.is_some()
    }

    /// Whether the process has ended.
    pub fn is_ended(&self) -> bool {
        self.shared.ended.load(Ordering::SeqCst)
    }

    fn ready(&self) -> Result<(ConnectionTo<Agent>, acp::SessionId), SessionError> {
        let connection = self.shared.connection().ok_or(SessionError::Closed)?;
        let session = self.shared.state().session.clone().ok_or(SessionError::Closed)?;
        Ok((connection, session))
    }

    /// Sends `message` as a prompt; the turn runs until the agent answers it.
    pub fn send(&self, message: UserMessage) -> Result<(), SessionError> {
        let (connection, session) = self.ready()?;
        let (blocks, updates) = {
            let mut state = self.shared.state();
            let blocks = prompt::blocks(&message, &state.caps, &self.shared.launch.cwd);
            (blocks, state.mapper.begin_turn(&message, now()))
        };
        self.shared.apply(updates);
        let shared = self.shared.clone();
        ask(connection, acp::PromptRequest::new(session, blocks), move |result| {
            let updates = {
                let mut state = shared.state();
                match &result {
                    Ok(response) => {
                        let mut updates = state.mapper.end_turn(&wire_name(&response.stop_reason), now());
                        updates.extend(crate::usage::response(response));
                        updates
                    }
                    Err(error) => {
                        let mut updates = state.mapper.error(ErrorKind::Api, error.message.clone(), now());
                        updates.push(Update::Status(Status::Idle));
                        updates
                    }
                }
            };
            shared.withdraw_asks();
            shared.apply(updates);
        });
        Ok(())
    }

    /// Asks the agent to stop the turn; open permission requests are withdrawn.
    pub fn cancel(&self) {
        let Ok((connection, session)) = self.ready() else {
            return;
        };
        self.shared.withdraw_asks();
        let _ = connection.send_notification(acp::CancelNotification::new(session));
    }

    /// Answers the permission request `request` with `choice`.
    pub fn answer(&self, request: &str, choice: Choice) -> Result<(), SessionError> {
        let ask = self.shared.state().asks.remove(request).ok_or(SessionError::Closed)?;
        let picked = picked(&ask.options, choice, ask.destructive);
        let (outcome, decision) = match picked {
            Some(option) => {
                let decision = match option.kind {
                    acp::PermissionOptionKind::AllowOnce => Decision::Allowed {
                        always: false,
                        edited: false,
                    },
                    acp::PermissionOptionKind::AllowAlways => Decision::Allowed {
                        always: true,
                        edited: false,
                    },
                    _ => Decision::Denied { message: None },
                };
                let selected = acp::SelectedPermissionOutcome::new(option.option_id.clone());
                (acp::RequestPermissionOutcome::Selected(selected), decision)
            }
            None => (
                acp::RequestPermissionOutcome::Cancelled,
                Decision::Denied { message: None },
            ),
        };
        ask.responder
            .respond(acp::RequestPermissionResponse::new(outcome))
            .map_err(|e| SessionError::Agent(e.message))?;
        let still_asking = !self.shared.state().asks.is_empty();
        let mut updates = vec![Update::Resolve {
            key: format!("permission-{request}"),
            state: RequestState::Decided(decision),
        }];
        if !still_asking {
            updates.push(Update::Status(Status::Running));
        }
        self.shared.apply(updates);
        Ok(())
    }

    /// Switches the session to mode `id`.
    pub fn set_mode(&self, id: &str) -> Result<(), SessionError> {
        let (connection, session) = self.ready()?;
        let shared = self.shared.clone();
        let mode = id.to_string();
        ask(
            connection,
            acp::SetSessionModeRequest::new(session, id.to_string()),
            move |result| match result {
                Ok(_) => {
                    shared.state().caps.mode = Some(mode.clone());
                    shared.apply(vec![Update::Mode(mode)]);
                    (shared.sink)(Event::Capabilities);
                }
                Err(error) => shared.fail(SessionError::Agent(error.message)),
            },
        );
        Ok(())
    }

    /// Sets the select option `option` to `value`, such as the model.
    pub fn set_option(&self, option: &str, value: &str) -> Result<(), SessionError> {
        self.configure(
            option,
            acp::SessionConfigOptionValue::ValueId {
                value: value.to_string().into(),
            },
        )
    }

    /// Turns the on/off option `option` on or off.
    pub fn set_flag(&self, option: &str, on: bool) -> Result<(), SessionError> {
        self.configure(option, acp::SessionConfigOptionValue::Boolean { value: on })
    }

    fn configure(&self, option: &str, value: acp::SessionConfigOptionValue) -> Result<(), SessionError> {
        let (connection, session) = self.ready()?;
        let shared = self.shared.clone();
        let request = acp::SetSessionConfigOptionRequest::new(session, option.to_string(), value);
        ask(connection, request, move |result| match result {
            Ok(response) => {
                let updates = {
                    let mut state = shared.state();
                    state.caps.learn_options(&response.config_options);
                    state.mapper.options_changed(&state.caps)
                };
                shared.apply(updates);
                (shared.sink)(Event::Capabilities);
            }
            Err(error) => shared.fail(SessionError::Agent(error.message)),
        });
        Ok(())
    }

    /// Signs in with `method`, one of the capabilities' methods the agent runs itself, and then opens the session.
    pub fn authenticate(&self, method: &str) -> Result<(), SessionError> {
        let connection = self.shared.connection().ok_or(SessionError::Closed)?;
        let shared = self.shared.clone();
        let later = connection.clone();
        ask(
            connection,
            acp::AuthenticateRequest::new(method.to_string()),
            move |result| match result {
                Ok(_) => {
                    let _ = std::thread::Builder::new().name("acp-open".into()).spawn(move || {
                        if let Err(error) = futures::executor::block_on(open(&shared, &later)) {
                            shared.fail(error);
                        }
                    });
                }
                Err(error) => shared.fail(SessionError::Agent(error.message)),
            },
        );
        Ok(())
    }

    /// Ends the session: the connection closes, then the supervisor stops the process after `grace`.
    pub fn stop(&self, grace: Duration) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        if let Some(shutdown) = self.shared.shutdown.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = shutdown.send(());
        }
        self.shared.supervisor.stop(self.shared.process, grace);
    }
}

/// Runs the connection over the agent's pipes until it closes or `closed` fires.
fn connect(shared: Arc<Shared>, stdin: ChildStdin, stdout: ChildStdout, closed: oneshot::Receiver<()>) {
    let transport = ByteStreams::new(blocking::Unblock::new(stdin), blocking::Unblock::new(stdout));
    let updates = shared.clone();
    let asks = shared.clone();
    let main = shared.clone();
    let result = futures::executor::block_on(
        // Unguarded, as the v1 guard turns an agent of another protocol version into an error that names the
        // version only in its text; `initialize` checks the version itself.
        Builder::new(Client)
            .name(ide_product::NAME)
            .on_receive_notification(
                async move |notification: acp::SessionNotification, _cx| {
                    updates.on_update(notification);
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: acp::RequestPermissionRequest, responder, _cx| {
                    asks.on_permission(request, responder);
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(transport, async move |connection: ConnectionTo<Agent>| {
                *main.connection.lock().unwrap_or_else(|e| e.into_inner()) = Some(connection.clone());
                match initialize(&main, &connection).await {
                    Ok(()) => match open(&main, &connection).await {
                        Ok(()) | Err(SessionError::SignIn) => {}
                        Err(error) => main.fail(error),
                    },
                    Err(error) => main.fail(error),
                }
                let _ = closed.await;
                Ok(())
            }),
    );
    shared.finish(result);
}

async fn initialize(shared: &Shared, connection: &ConnectionTo<Agent>) -> Result<(), SessionError> {
    let request = acp::InitializeRequest::new(ProtocolVersion::V1)
        .client_info(acp::Implementation::new(ide_product::NAME, env!("CARGO_PKG_VERSION")));
    let response = connection
        .send_request(request)
        .block_task()
        .await
        .map_err(|e| SessionError::Agent(e.message))?;
    let version = response.protocol_version.as_u16();
    shared.state().caps.learn_initialize(&response);
    (shared.sink)(Event::Capabilities);
    if version != ProtocolVersion::V1.as_u16() {
        return Err(SessionError::Version(version));
    }
    Ok(())
}

/// Opens the session: loads the one to continue where the agent can, else starts a new one.
async fn open(shared: &Shared, connection: &ConnectionTo<Agent>) -> Result<(), SessionError> {
    let launch = &shared.launch;
    let meta = launch.session_meta.as_ref().and_then(|m| m.as_object().cloned());
    let can_load = shared.state().caps.load_session;
    let opened = match launch.resume.clone().filter(|_| can_load) {
        Some(id) => {
            let request = acp::LoadSessionRequest::new(id.clone(), launch.cwd.clone()).meta(meta);
            let session = acp::SessionId::new(id);
            shared.state().session = Some(session.clone());
            // The agent replays the whole conversation while it loads, so what the thread showed goes first.
            shared.apply(vec![Update::Clear]);
            connection
                .send_request(request)
                .block_task()
                .await
                .map(|r| (session, r.modes, r.config_options))
        }
        None => {
            let request = acp::NewSessionRequest::new(launch.cwd.clone()).meta(meta);
            connection
                .send_request(request)
                .block_task()
                .await
                .map(|r| (r.session_id, r.modes, r.config_options))
        }
    };
    let (session, modes, options) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            shared.state().session = None;
            if error.code == acp::ErrorCode::AuthRequired {
                shared.fail(SessionError::SignIn);
                return Err(SessionError::SignIn);
            }
            return Err(SessionError::Agent(error.message));
        }
    };
    let updates = {
        let mut state = shared.state();
        let State { mapper, caps, .. } = &mut *state;
        caps.learn_modes(modes.as_ref());
        caps.learn_options(options.as_deref().unwrap_or_default());
        let mut updates = mapper.settle(now());
        updates.push(Update::Session(session.0.to_string()));
        if let Some(mode) = &caps.mode {
            updates.push(Update::Mode(mode.clone()));
        }
        updates.extend(mapper.options_changed(caps));
        updates.push(Update::Status(Status::Idle));
        state.session = Some(session);
        updates
    };
    shared.apply(updates);
    (shared.sink)(Event::Capabilities);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn options() -> Vec<acp::PermissionOption> {
        serde_json::from_value(json!([
            {"optionId": "allow", "name": "Allow", "kind": "allow_once"},
            {"optionId": "always", "name": "Always allow git", "kind": "allow_always"},
            {"optionId": "reject", "name": "Reject", "kind": "reject_once"}
        ]))
        .unwrap()
    }

    #[test]
    fn allowing_a_destructive_git_command_always_stores_no_rule() {
        let options = options();
        let id = |o: Option<&acp::PermissionOption>| o.map(|o| o.option_id.0.to_string());
        assert_eq!(id(picked(&options, Choice::Option(1), true)), Some("allow".into()));
        assert_eq!(id(picked(&options, Choice::Option(1), false)), Some("always".into()));
        assert_eq!(id(picked(&options, Choice::Option(2), true)), Some("reject".into()));
        assert_eq!(id(picked(&options, Choice::Dismiss, true)), None);
        let only_always: Vec<acp::PermissionOption> = options.into_iter().skip(1).collect();
        assert_eq!(id(picked(&only_always, Choice::Option(0), true)), None);
    }
}
