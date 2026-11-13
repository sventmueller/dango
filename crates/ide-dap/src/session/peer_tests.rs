//! Sessions against an adapter written out here, on a loopback port as `dlv dap` and js-debug listen: the events and
//! requests the scripted adapter never sends, a child session on a connection of its own, and stopping both.

use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use ide_process::Supervisor;
use serde_json::{Map, Value, json};

use super::{Event, Session, Setup, wait};
use crate::adapter::{Adapter, AdapterKind};
use crate::error::Error;
use crate::launch::LaunchConfig;
use crate::protocol::{self, Message};
use crate::types::{EndReason, Request, SourceBreakpoint, State, StopReason, ThreadId};

const WAIT: Duration = Duration::from_secs(10);

/// Which side of the adapter a connection plays.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// The first connection, which starts a child session.
    Parent,
    /// The connection the child session dials.
    Child,
    /// The only connection of an adapter on two streams, which then breaks off.
    Single,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Parent => "parent",
            Role::Child => "child",
            Role::Single => "single",
        }
    }
}

/// One connection of the adapter: it answers the client's requests and tells `log` what it saw.
struct Peer {
    out: TcpStream,
    seq: i64,
    role: Role,
    log: Sender<String>,
}

impl Peer {
    fn send(&mut self, message: Message) {
        let _ = self.out.write_all(&message.to_frame());
        let _ = self.out.flush();
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn event(&mut self, event: &str, body: Value) {
        let seq = self.next_seq();
        self.send(Message::Event {
            seq,
            event: event.into(),
            body,
        });
    }

    fn respond(&mut self, request_seq: i64, command: &str, body: Value) {
        let seq = self.next_seq();
        self.send(Message::Response {
            seq,
            request_seq,
            success: true,
            command: command.into(),
            message: None,
            body,
        });
    }

    fn request(&mut self, command: &str, arguments: Value) {
        let seq = self.next_seq();
        self.send(Message::Request {
            seq,
            command: command.into(),
            arguments,
        });
    }

    fn note(&self, what: String) {
        let _ = self.log.send(format!("{} {what}", self.role.name()));
    }

    /// Answers the client until it says goodbye or, for `Role::Single`, until it answered `startDebugging`.
    fn serve(mut self, input: TcpStream, go: Option<Receiver<()>>) {
        let mut input = BufReader::new(input);
        while let Ok(Some(body)) = ide_process::framing::read(&mut input) {
            match protocol::parse(&body) {
                Ok(Message::Request {
                    seq,
                    command,
                    arguments,
                }) => {
                    if command == "initialize"
                        && let Some(go) = &go
                    {
                        let _ = go.recv_timeout(WAIT);
                    }
                    if !self.answer(seq, &command, &arguments) {
                        return;
                    }
                }
                Ok(Message::Response {
                    command,
                    success,
                    message,
                    body,
                    ..
                }) => {
                    let said = message.unwrap_or_else(|| body.to_string());
                    self.note(format!("answer {command} {success} {said}"));
                    if self.role == Role::Single && command == "startDebugging" {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Answers one request; false once the client said goodbye.
    fn answer(&mut self, seq: i64, command: &str, arguments: &Value) -> bool {
        match command {
            "initialize" => {
                let capabilities = if self.role == Role::Parent {
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "exceptionBreakpointFilters": [{"filter": "panic", "label": "Panics", "default": true}],
                    })
                } else {
                    json!({})
                };
                self.respond(seq, command, capabilities);
                self.event("initialized", Value::Null);
            }
            "launch" => {
                self.note(format!("launch {}", arguments["name"].as_str().unwrap_or_default()));
                self.respond(seq, command, Value::Null);
                self.launched();
            }
            "setBreakpoints" => {
                let lines: Vec<Value> = arguments["breakpoints"]
                    .as_array()
                    .map(|list| list.iter().map(|b| b["line"].clone()).collect())
                    .unwrap_or_default();
                let name = arguments["source"]["path"].as_str().unwrap_or_default();
                let name = Path::new(name).file_name().unwrap_or_default().to_string_lossy();
                self.note(format!("setBreakpoints {name} {lines:?}"));
                let answered: Vec<Value> = lines.iter().map(|l| json!({"verified": true, "line": l})).collect();
                self.respond(seq, command, json!({"breakpoints": answered}));
            }
            "setExceptionBreakpoints" => {
                self.note(format!("setExceptionBreakpoints {}", arguments["filters"]));
                self.respond(seq, command, Value::Null);
            }
            "threads" => self.respond(seq, command, json!({"threads": [{"id": 1, "name": "main"}]})),
            "pause" => {
                self.respond(seq, command, Value::Null);
                self.event("stopped", json!({"reason": "pause", "threadId": 1}));
            }
            "continue" => {
                self.event("continued", json!({"threadId": 1, "allThreadsContinued": false}));
                self.respond(seq, command, json!({"allThreadsContinued": false}));
                self.event("invalidated", json!({"areas": ["variables"]}));
                self.event("thread", json!({"reason": "exited", "threadId": 2}));
            }
            "terminate" => {
                self.note("terminate".into());
                self.respond(seq, command, Value::Null);
                self.event("exited", json!({"exitCode": 0}));
                self.event("terminated", Value::Null);
            }
            "disconnect" => {
                self.note(format!("disconnect {}", arguments["terminateDebuggee"]));
                self.respond(seq, command, Value::Null);
                return false;
            }
            other => {
                self.note(other.to_string());
                self.respond(seq, command, Value::Null);
            }
        }
        true
    }

    /// What the adapter tells and asks once the program runs.
    fn launched(&mut self) {
        match self.role {
            Role::Parent => {
                self.event("process", json!({"name": "/p/app", "startMethod": "launch"}));
                self.event("thread", json!({"reason": "started", "threadId": 2}));
                self.event("output", json!({"category": "stderr", "output": "warming up\n"}));
                self.event(
                    "breakpoint",
                    json!({"reason": "changed", "breakpoint": {"id": 4, "verified": true, "line": 3}}),
                );
                self.event(
                    "capabilities",
                    json!({"capabilities": {"supportsRestartRequest": true}}),
                );
                self.request("runInTerminal", json!({"cwd": "/p", "args": []}));
                self.request(
                    "runInTerminal",
                    json!({"kind": "external", "title": "app", "cwd": "/p", "args": ["/p/app", "--x"],
                        "env": {"A": "1", "B": null}}),
                );
                self.request("frobnicate", Value::Null);
                self.request(
                    "startDebugging",
                    json!({"request": "launch", "configuration": {"name": "worker", "program": "/p/worker"}}),
                );
            }
            Role::Single => self.request("startDebugging", json!({"request": "attach", "configuration": {}})),
            Role::Child => {}
        }
    }
}

fn peer(stream: &TcpStream, role: Role, log: &Sender<String>) -> Peer {
    Peer {
        out: stream.try_clone().unwrap(),
        seq: 0,
        role,
        log: log.clone(),
    }
}

/// A loopback port on which the first connection plays the parent and the second the child.
fn listen(log: &Sender<String>, go: Receiver<()>) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = log.clone();
    std::thread::spawn(move || {
        let mut go = Some(go);
        for (index, stream) in listener.incoming().take(2).enumerate() {
            let Ok(stream) = stream else { return };
            let role = if index == 0 { Role::Parent } else { Role::Child };
            let peer = peer(&stream, role, &log);
            let go = go.take();
            std::thread::spawn(move || peer.serve(stream, go));
        }
    });
    port
}

fn config(name: &str) -> LaunchConfig {
    LaunchConfig {
        name: name.into(),
        kind: AdapterKind::Lldb,
        request: Request::Launch,
        arguments: Map::from_iter([("program".to_string(), json!("/p/app"))]),
        process_pick: false,
    }
}

fn next<T>(rx: &Receiver<(u64, Event)>, mut pick: impl FnMut(u64, Event) -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let (id, event) = rx.recv_timeout(left).expect("the event did not come");
        if let Some(found) = pick(id, event) {
            return found;
        }
    }
}

/// What the adapter's connections logged, in order.
struct Logged {
    rx: Receiver<String>,
    lines: Vec<String>,
}

impl Logged {
    /// Waits until the log has `line`; everything logged so far.
    fn until(&mut self, line: &str) -> &[String] {
        while !self.lines.iter().any(|l| l == line) {
            match self.rx.recv_timeout(WAIT) {
                Ok(entry) => self.lines.push(entry),
                Err(_) => panic!("{line:?} was not logged, only {:?}", self.lines),
            }
        }
        &self.lines
    }

    fn position(&self, line: &str) -> Option<usize> {
        self.lines.iter().position(|l| l == line)
    }
}

fn breakpoint(line: u32) -> SourceBreakpoint {
    SourceBreakpoint {
        line,
        ..SourceBreakpoint::default()
    }
}

type Events = Receiver<(u64, Event)>;

/// A session on the adapter of `port`, a stand-in process for which waits; with its events.
fn start_listening(port: u16) -> (Session, Events) {
    let (tx, rx) = channel();
    let cwd = std::env::temp_dir();
    let adapter = Adapter::listening(
        AdapterKind::Lldb,
        Path::new("/bin/sleep"),
        &["60".into()],
        Vec::new(),
        &cwd,
        port,
    );
    let session = Session::start(
        &Supervisor::new(),
        adapter,
        config("app"),
        Setup::default(),
        move |id, event| {
            let _ = tx.send((id.get(), event));
        },
    )
    .unwrap();
    (session, rx)
}

/// Before the adapter answers `initialize`: breakpoints and filters are kept, the program can be neither paused nor
/// restarted.
fn before_the_answer(session: &Session, main: &Path) {
    let early = wait(|reply| session.set_breakpoints(main, vec![breakpoint(3)], reply)).unwrap();
    assert_eq!((early[0].line, early[0].verified), (Some(3), false));
    assert_eq!(
        wait(|reply| session.set_exception_filters(vec!["panic".into()], reply)),
        Ok(())
    );
    assert_eq!(wait(|reply| session.pause(ThreadId(1), reply)), Err(Error::NotStopped));
    assert_eq!(wait(|reply| session.restart(reply)), Err(Error::Unsupported));
}

/// What the adapter reports once the program runs, up to the child session it starts, which it returns.
fn reported(session: &Session, rx: &Events) -> Session {
    let program = next(rx, |_, e| match e {
        Event::Program(program) => Some(program),
        _ => None,
    });
    assert_eq!((program.name.as_str(), program.pid), ("/p/app", None));
    let output = next(rx, |_, e| match e {
        Event::Output(output) => Some(output.text),
        _ => None,
    });
    assert_eq!(output, "warming up\n");
    let changed = next(rx, |_, e| match e {
        Event::Breakpoint(breakpoint) => Some(breakpoint),
        _ => None,
    });
    assert_eq!((changed.id, changed.line), (Some(4), Some(3)));
    let restart = next(rx, |_, e| match e {
        Event::Capabilities(capabilities) => Some(capabilities.restart),
        _ => None,
    });
    assert!(restart && session.capabilities().restart && session.capabilities().terminate);
    let (request, answer) = next(rx, |_, e| match e {
        Event::RunInTerminal(request, answer) => Some((request, answer)),
        _ => None,
    });
    assert_eq!(
        (request.title.as_deref(), request.cwd.as_path(), request.external),
        (Some("app"), Path::new("/p"), true)
    );
    assert_eq!(request.args, ["/p/app", "--x"]);
    assert_eq!(
        request.env,
        [("A".to_string(), Some("1".to_string())), ("B".to_string(), None)]
    );
    assert_eq!(format!("{answer:?}"), "TerminalAnswer");
    answer.started(Some(10), Some(11));
    let child = next(rx, |_, e| match e {
        Event::Child(child) => Some(child),
        _ => None,
    });
    assert_eq!((child.name(), child.request()), ("worker", Request::Launch));
    let child_id = child.id().get();
    next(rx, |id, e| {
        (id == child_id && matches!(e, Event::Running)).then_some(())
    });
    assert!(session.wait_for(WAIT, |s| *s == State::Running));
    child
}

/// Pausing, the threads, restarting and continuing, with the events the adapter sends along.
fn paused_and_continued(session: &Session, rx: &Events) {
    assert_eq!(wait(|reply| session.pause(ThreadId(1), reply)), Ok(()));
    let stop = next(rx, |id, e| match e {
        Event::Stopped(stop) if id == session.id().get() => Some(stop),
        _ => None,
    });
    assert_eq!((stop.reason, stop.thread), (StopReason::Pause, Some(ThreadId(1))));
    let threads = wait(|reply| session.threads(reply)).unwrap();
    assert_eq!(threads, session.known_threads());
    assert_eq!(wait(|reply| session.restart(reply)), Ok(()));
    assert_eq!(wait(|reply| session.resume(ThreadId(1), reply)), Ok(()));
    let continued = next(rx, |_, e| match e {
        Event::Continued { thread, all_threads } => Some((thread, all_threads)),
        _ => None,
    });
    assert_eq!(continued, (Some(ThreadId(1)), false));
    next(rx, |_, e| matches!(e, Event::Invalidated).then_some(()));
    let exited = next(rx, |_, e| match e {
        Event::ThreadExited(thread) => Some(thread),
        _ => None,
    });
    assert_eq!(exited, ThreadId(2));
}

#[test]
fn a_listening_adapter_reports_its_program_and_starts_a_child_that_ends_with_its_parent() {
    let (log_tx, rx_log) = channel();
    let mut log = Logged {
        rx: rx_log,
        lines: Vec::new(),
    };
    let (go, wait_for_go) = channel();
    let (session, rx) = start_listening(listen(&log_tx, wait_for_go));
    let main = PathBuf::from("/p/src/main.rs");
    before_the_answer(&session, &main);
    go.send(()).unwrap();
    let child = reported(&session, &rx);

    let seen = log.until("parent answer startDebugging true null").to_vec();
    for line in [
        "parent setBreakpoints main.rs [Number(3)]",
        "parent setExceptionBreakpoints [\"panic\"]",
        "parent answer runInTerminal false runInTerminal without a program",
        "parent answer frobnicate false not supported",
    ] {
        assert!(seen.iter().any(|l| l == line), "{line:?} missing from {seen:?}");
    }
    log.until("child launch worker");
    log.until("parent answer runInTerminal true {\"processId\":10,\"shellProcessId\":11}");

    let other = PathBuf::from("/p/src/other.rs");
    let set = wait(|reply| session.set_breakpoints(&other, vec![breakpoint(7)], reply)).unwrap();
    assert!(set[0].verified);
    assert_eq!(wait(|reply| session.set_exception_filters(Vec::new(), reply)), Ok(()));
    log.until("child setBreakpoints other.rs [Number(7)]");
    log.until("child setExceptionBreakpoints []");
    paused_and_continued(&session, &rx);

    assert_eq!(session.label(), "sleep");
    assert_eq!(
        (session.kind(), session.config().name.as_str()),
        (AdapterKind::Lldb, "app")
    );
    assert!(session.adapter_process().is_some() && child.adapter_process().is_none());
    assert!(session.same(&session.clone()) && !session.same(&child));
    assert_eq!(session.children().len(), 1);
    assert_eq!(session.setup().breakpoints.keys().collect::<Vec<_>>(), [&main, &other]);
    assert!(format!("{session:?}").contains("\"app\""));

    session.stop();
    assert!(session.wait_until_ended(WAIT * 2) && child.wait_until_ended(WAIT));
    let end = session.end().unwrap();
    assert_eq!((end.reason, end.exit_code), (EndReason::Stopped, Some(0)));
    log.until("parent disconnect true");
    let (child_gone, terminate) = (log.position("child disconnect true"), log.position("parent terminate"));
    assert!(child_gone.is_some() && child_gone < terminate, "{:?}", log.lines);
}

#[test]
fn an_adapter_on_one_connection_cannot_start_a_child_and_its_breaking_off_is_a_crash() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let ours = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (theirs, _) = listener.accept().unwrap();
    let (log_tx, rx_log) = channel();
    let mut log = Logged {
        rx: rx_log,
        lines: Vec::new(),
    };
    let single = peer(&theirs, Role::Single, &log_tx);
    std::thread::spawn(move || single.serve(theirs, None));
    let adapter = Adapter::streams(AdapterKind::Lldb, "peer", ours.try_clone().unwrap(), ours);
    let session = Session::start(&Supervisor::new(), adapter, config("app"), Setup::default(), |_, _| {}).unwrap();

    log.until("single answer startDebugging false child sessions need an adapter that takes several connections");
    assert!(session.wait_until_ended(WAIT));
    assert_eq!(
        session.end().unwrap().reason,
        EndReason::AdapterCrashed {
            ending: None,
            stderr: String::new()
        }
    );
    assert!(session.children().is_empty());
}
