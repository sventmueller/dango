//! One debug session: its adapter, started through the supervisor or reached over a stream, initialized, configured
//! with the breakpoints, and stopped by who owns the program (Review C7).

mod event;
mod handler;
mod requests;
mod start;
mod stop;

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use ide_process::tail::Tail;
use ide_process::{ProcessId, Supervisor};

use crate::adapter::{Adapter, AdapterKind};
use crate::connection::Connection;
use crate::error::Error;
use crate::launch::LaunchConfig;
use crate::types::{Capabilities, End, EndReason, Program, Request, SessionId, SourceBreakpoint, State, Thread};

pub use event::{Event, TerminalAnswer};
pub use stop::stop_all;

/// How long the adapter may take to answer `initialize`, `disconnect` and the like.
const SHORT: Duration = Duration::from_secs(30);

/// How long launching or attaching may take; an adapter may build or load a large program first.
const LONG: Duration = Duration::from_secs(10 * 60);

/// How much of an adapter's standard error is kept for the crash message.
const STDERR_KEPT: usize = 16 * 1024;

/// What a session is set up with before the program runs; later changes go through `Session::set_breakpoints` and
/// `Session::set_exception_filters`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Setup {
    /// The breakpoints, by file.
    pub breakpoints: BTreeMap<PathBuf, Vec<SourceBreakpoint>>,
    /// The exception filters to turn on, by `ExceptionFilter::filter`; `None` turns on those the adapter marks as
    /// default.
    pub exception_filters: Option<Vec<String>>,
}

type Sink = Arc<dyn Fn(SessionId, Event) + Send + Sync>;

/// How a child session reaches the adapter of its parent.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Reach {
    /// The adapter listens on this port and takes a connection per session.
    Port(u16),
    /// The adapter has only the one connection.
    Single,
}

pub(crate) struct Shared {
    pub(crate) state: State,
    pub(crate) capabilities: Capabilities,
    pub(crate) connection: Option<Arc<Connection>>,
    /// The adapter process; only a session that started it has one.
    pub(crate) adapter: Option<ProcessId>,
    pub(crate) stderr: Arc<Mutex<Tail>>,
    pub(crate) program: Option<Program>,
    /// The adapter asked for `runInTerminal`, so the program belongs to a terminal tab.
    pub(crate) in_terminal: bool,
    pub(crate) exit_code: Option<i64>,
    pub(crate) terminated: bool,
    pub(crate) launched: bool,
    pub(crate) configured: bool,
    pub(crate) stopping: bool,
    /// How the session ends once the connection or the adapter goes, when that was decided before.
    pub(crate) intended: Option<EndReason>,
    /// `terminate` and `disconnect` were sent, by the user's stop or the supervisor's graceful step.
    pub(crate) said_goodbye: bool,
    /// Counts `stopped` events, so a step's late response does not undo a stop that came before it.
    pub(crate) stops: u64,
    pub(crate) threads: Vec<Thread>,
    pub(crate) setup: Setup,
    pub(crate) children: Vec<Session>,
    pub(crate) reach: Reach,
}

pub(crate) struct Inner {
    pub(crate) id: SessionId,
    pub(crate) config: LaunchConfig,
    pub(crate) kind: AdapterKind,
    pub(crate) label: String,
    pub(crate) supervisor: Supervisor,
    pub(crate) sink: Sink,
    pub(crate) shared: Mutex<Shared>,
    pub(crate) changed: Condvar,
    pub(crate) record: Option<PathBuf>,
}

impl Inner {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn emit(&self, event: Event) {
        let sink = self.sink.clone();
        crate::connection::contained(|| sink(self.id, event));
    }

    pub(crate) fn connection(&self) -> Result<Arc<Connection>, Error> {
        let shared = self.lock();
        if matches!(shared.state, State::Ended(_)) {
            return Err(Error::Ended);
        }
        shared.connection.clone().ok_or(Error::Ended)
    }
}

/// The external handle; when the last one goes, the session is stopped.
struct Handle {
    inner: Arc<Inner>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let ended = matches!(self.inner.lock().state, State::Ended(_));
        if !ended {
            stop::begin(&self.inner);
        }
    }
}

/// A debug session; clones share it, and dropping the last one stops it.
///
/// Calls do not block: requests answer through a `Reply` on a background thread, events arrive through the callback
/// given to `start`.
#[derive(Clone)]
pub struct Session {
    handle: Arc<Handle>,
}

static NEXT: AtomicU64 = AtomicU64::new(1);

impl Session {
    /// Starts `adapter` and launches or attaches as `config` says, with the breakpoints and exception filters of
    /// `setup`; `events` hears everything that happens, on a background thread, and must return quickly.
    ///
    /// The caller has decided that the project is trusted: the adapter runs the project's program. `ide_dap::start`
    /// does that check and finds the adapter.
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    /// use std::time::Duration;
    ///
    /// use ide_dap::{Adapter, Event, Session, Setup, State, fake};
    /// use ide_process::Supervisor;
    ///
    /// let script = fake::Script::default();
    /// let config = script.launch_config();
    /// let mut setup = Setup::default();
    /// setup.breakpoints.insert(script.path.clone(), vec![script.breakpoint(3)]);
    /// let (tx, rx) = channel();
    /// let session = Session::start(Supervisor::global(), Adapter::fake(script), config, setup, move |_, event| {
    ///     let _ = tx.send(event);
    /// })?;
    /// while let Ok(event) = rx.recv_timeout(Duration::from_secs(5)) {
    ///     if let Event::Stopped(stop) = event {
    ///         assert_eq!(stop.hit_breakpoints.len(), 1);
    ///         break;
    ///     }
    /// }
    /// assert!(matches!(session.state(), State::Stopped(_)));
    /// session.stop();
    /// assert!(session.wait_until_ended(Duration::from_secs(5)));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn start(
        supervisor: &Supervisor,
        adapter: Adapter,
        config: LaunchConfig,
        setup: Setup,
        events: impl Fn(SessionId, Event) + Send + Sync + 'static,
    ) -> io::Result<Session> {
        Session::begin(supervisor, adapter, config, setup, Arc::new(events))
    }

    fn begin(
        supervisor: &Supervisor,
        adapter: Adapter,
        config: LaunchConfig,
        setup: Setup,
        sink: Sink,
    ) -> io::Result<Session> {
        let inner = Arc::new(Inner {
            id: SessionId(NEXT.fetch_add(1, Ordering::Relaxed)),
            kind: adapter.kind,
            label: adapter.label.clone(),
            config,
            supervisor: supervisor.clone(),
            sink,
            shared: Mutex::new(Shared {
                state: State::Starting,
                capabilities: Capabilities::default(),
                connection: None,
                adapter: None,
                stderr: Arc::new(Mutex::new(Tail::new(STDERR_KEPT))),
                program: None,
                in_terminal: false,
                exit_code: None,
                terminated: false,
                launched: false,
                configured: false,
                stopping: false,
                intended: None,
                said_goodbye: false,
                stops: 0,
                threads: Vec::new(),
                setup,
                children: Vec::new(),
                reach: Reach::Single,
            }),
            changed: Condvar::new(),
            record: adapter.record.clone(),
        });
        start::connect(&inner, adapter)?;
        stop::remember(&inner);
        start::initialize(&inner);
        Ok(Session {
            handle: Arc::new(Handle { inner }),
        })
    }

    pub(crate) fn inner(&self) -> &Arc<Inner> {
        &self.handle.inner
    }

    /// Its number, which every event carries.
    pub fn id(&self) -> SessionId {
        self.inner().id
    }

    /// The name of its launch configuration.
    pub fn name(&self) -> &str {
        &self.inner().config.name
    }

    /// The configuration it was started with.
    pub fn config(&self) -> &LaunchConfig {
        &self.inner().config
    }

    /// Which adapter it runs on.
    pub fn kind(&self) -> AdapterKind {
        self.inner().kind
    }

    /// What the activity view shows for its adapter.
    pub fn label(&self) -> &str {
        &self.inner().label
    }

    /// Whether it launched the program or attached to it: the stop button reads "Stop" or "Detach".
    pub fn request(&self) -> Request {
        self.inner().config.request
    }

    /// Where it is in its life.
    pub fn state(&self) -> State {
        self.inner().lock().state.clone()
    }

    /// What its adapter can do; empty until `Event::Initialized`.
    pub fn capabilities(&self) -> Capabilities {
        self.inner().lock().capabilities.clone()
    }

    /// The program, once the adapter described it.
    pub fn program(&self) -> Option<Program> {
        self.inner().lock().program.clone()
    }

    /// The threads the last `threads` answer listed.
    pub fn known_threads(&self) -> Vec<Thread> {
        self.inner().lock().threads.clone()
    }

    /// The sessions the adapter started beneath this one.
    pub fn children(&self) -> Vec<Session> {
        self.inner().lock().children.clone()
    }

    /// The supervisor's number for the adapter process, if this session started one.
    pub fn adapter_process(&self) -> Option<ProcessId> {
        self.inner().lock().adapter
    }

    /// The breakpoints and filters it holds now.
    pub fn setup(&self) -> Setup {
        self.inner().lock().setup.clone()
    }

    /// Whether `other` is a handle of the same session.
    pub fn same(&self, other: &Session) -> bool {
        Arc::ptr_eq(self.inner(), other.inner())
    }

    /// How it ended, once it has.
    pub fn end(&self) -> Option<End> {
        match &self.inner().lock().state {
            State::Ended(end) => Some(end.clone()),
            _ => None,
        }
    }

    /// Waits until `done` holds for the state or `timeout` passed; for background threads and tests.
    pub fn wait_for(&self, timeout: Duration, done: impl Fn(&State) -> bool) -> bool {
        let inner = self.inner();
        let deadline = Instant::now() + timeout;
        let mut shared = inner.lock();
        loop {
            if done(&shared.state) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            shared = inner
                .changed
                .wait_timeout(shared, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Waits until the session ended or `timeout` passed; for background threads and tests.
    pub fn wait_until_ended(&self, timeout: Duration) -> bool {
        self.wait_for(timeout, |state| matches!(state, State::Ended(_)))
    }

    /// Stops the session in the background by who owns the program (Review C7): a launched one is asked to end with
    /// `terminate`, then `disconnect`, and last the adapter's process group is ended; an attached one is detached
    /// with `disconnect` and never gets a signal. `Event::Ended` follows.
    pub fn stop(&self) {
        stop::begin(self.inner());
    }

    /// Stops the session as `stop` does and waits up to `timeout` for the end; for quitting the app.
    pub fn stop_and_wait(&self, timeout: Duration) -> bool {
        self.stop();
        self.wait_until_ended(timeout)
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id())
            .field("name", &self.name())
            .field("kind", &self.kind())
            .field("state", &self.state())
            .finish()
    }
}

/// Waits for a request started with `start`; for background threads and tests, never for the main thread.
pub fn wait<T: Send + 'static>(start: impl FnOnce(crate::types::Reply<T>)) -> Result<T, Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    start(Box::new(move |answer| {
        let _ = tx.send(answer);
    }));
    rx.recv().unwrap_or(Err(Error::Ended))
}

pub(crate) fn weak(inner: &Arc<Inner>) -> Weak<Inner> {
    Arc::downgrade(inner)
}
