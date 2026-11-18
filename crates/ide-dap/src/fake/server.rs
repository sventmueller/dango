//! The fake adapter's side of the protocol.

use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Script, spawn_args};
use crate::protocol::{self, Message};
use crate::types::SourceBreakpoint;

const MAIN: i64 = 1;
const WORKER: i64 = 2;
const POINT: i64 = 1000;
const INNER: i64 = 1001;

struct Set {
    id: i64,
    asked: SourceBreakpoint,
    hits: u32,
}

/// Where the pretend program is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    NotStarted,
    /// Stopped before running the script's line at this index.
    At(usize),
    /// Stopped by the panic after the last line.
    Panicked,
    Ended,
}

struct Fake<W: Write> {
    script: Script,
    out: W,
    seq: i64,
    breakpoints: BTreeMap<PathBuf, Vec<Set>>,
    next_breakpoint: i64,
    filters: Vec<String>,
    place: Place,
    may_spawn: bool,
    child: Option<Child>,
    /// The launch request that waits for the app's answer to `runInTerminal`.
    waiting_launch: Option<(i64, String)>,
    /// `configurationDone` came while the launch waited for the terminal; the program runs once it started.
    begin_after_launch: bool,
    /// A `launch` or `attach` held back by `Script::launch_delay_ms`: when it is due, its number, command and arguments.
    held: Option<(Instant, i64, String, Value)>,
}

/// Serves `script` on `input` and `output` until the client disconnects or the input ends; `may_spawn` lets a
/// launch with a `spawn` argument (the program and its arguments) start that program for real and own it.
pub fn serve(script: Script, input: impl Read + Send + 'static, output: impl Write, may_spawn: bool) {
    let mut fake = Fake {
        script,
        out: output,
        seq: 1,
        breakpoints: BTreeMap::new(),
        next_breakpoint: 1,
        filters: vec!["panic".to_string()],
        place: Place::NotStarted,
        may_spawn,
        child: None,
        waiting_launch: None,
        begin_after_launch: false,
        held: None,
    };
    let (tx, rx) = channel();
    let reader = std::thread::Builder::new().name("dap-fake-read".into()).spawn(move || {
        let mut input = BufReader::new(input);
        while let Ok(Some(body)) = ide_process::framing::read(&mut input) {
            if tx.send(body).is_err() {
                return;
            }
        }
    });
    if reader.is_err() {
        return;
    }
    loop {
        let body = match fake
            .held
            .as_ref()
            .map(|held| held.0.saturating_duration_since(Instant::now()))
        {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(body) => body,
                Err(RecvTimeoutError::Timeout) => {
                    fake.release();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(body) => body,
                Err(_) => break,
            },
        };
        if !fake.receive(&body) {
            break;
        }
    }
    fake.end_child();
}

impl<W: Write> Fake<W> {
    /// Handles one message from the client; `false` ends the adapter.
    fn receive(&mut self, body: &[u8]) -> bool {
        let Ok(message) = protocol::parse(body) else {
            return true;
        };
        match message {
            Message::Request {
                seq,
                command,
                arguments,
            } => self.request(seq, &command, &arguments),
            Message::Response { body, success, .. } => {
                self.terminal_answered(success, &body);
                true
            }
            Message::Event { .. } => true,
        }
    }

    /// Answers the held `launch` or `attach` now.
    fn release(&mut self) {
        let Some((_, seq, command, arguments)) = self.held.take() else {
            return;
        };
        if command == "attach" {
            self.attach(seq, &arguments);
        } else {
            self.launch(seq, &arguments);
        }
        if self.waiting_launch.is_none() && std::mem::take(&mut self.begin_after_launch) {
            self.begin();
        }
    }

    fn attach(&mut self, seq: i64, arguments: &Value) {
        let pid = arguments.get("pid").cloned().unwrap_or(Value::Null);
        self.respond(seq, "attach", Ok(Value::Null));
        self.event(
            "process",
            json!({"name": "attached", "systemProcessId": pid, "startMethod": "attach"}),
        );
    }

    fn send(&mut self, message: Message) {
        let _ = self.out.write_all(&message.to_frame());
        let _ = self.out.flush();
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn respond(&mut self, request_seq: i64, command: &str, answer: Result<Value, String>) {
        let seq = self.next_seq();
        let (success, message, body) = match answer {
            Ok(body) => (true, None, body),
            Err(message) => (false, Some(message), Value::Null),
        };
        self.send(Message::Response {
            seq,
            request_seq,
            success,
            command: command.to_string(),
            message,
            body,
        });
    }

    fn event(&mut self, event: &str, body: Value) {
        let seq = self.next_seq();
        self.send(Message::Event {
            seq,
            event: event.to_string(),
            body,
        });
    }

    /// Answers one request; `false` ends the adapter.
    fn request(&mut self, seq: i64, command: &str, arguments: &Value) -> bool {
        let answer = match command {
            "initialize" => {
                let capabilities = self.capabilities();
                self.respond(seq, command, Ok(capabilities));
                self.event("initialized", Value::Null);
                return true;
            }
            "launch" | "attach" if self.script.launch_delay_ms > 0 => {
                let due = Instant::now() + Duration::from_millis(self.script.launch_delay_ms);
                self.held = Some((due, seq, command.to_string(), arguments.clone()));
                return true;
            }
            "launch" => return self.launch(seq, arguments),
            "attach" => {
                self.attach(seq, arguments);
                return true;
            }
            "setBreakpoints" => Ok(self.set_breakpoints(arguments)),
            "setExceptionBreakpoints" => {
                self.filters = strings(arguments.get("filters"));
                Ok(Value::Null)
            }
            "configurationDone" => {
                self.respond(seq, command, Ok(Value::Null));
                if self.waiting_launch.is_some() || self.held.is_some() {
                    self.begin_after_launch = true;
                } else {
                    self.begin();
                }
                return true;
            }
            "threads" => Ok(json!({"threads": [{"id": MAIN, "name": "main"}, {"id": WORKER, "name": "worker"}]})),
            "stackTrace" => Ok(self.stack_trace(arguments)),
            "scopes" => Ok(scopes(arguments)),
            "variables" => Ok(self.variables(arguments)),
            "evaluate" => self.evaluate(arguments),
            "continue" | "next" | "stepIn" | "stepOut" | "pause" | "restart" => {
                return self.execute(seq, command);
            }
            "terminate" => {
                self.respond(seq, command, Ok(Value::Null));
                self.end_child();
                self.finish();
                return true;
            }
            "disconnect" => {
                self.respond(seq, command, Ok(Value::Null));
                if arguments.get("terminateDebuggee").and_then(Value::as_bool) == Some(true) {
                    self.end_child();
                }
                return false;
            }
            _ => Err(format!("{command} is not supported")),
        };
        self.respond(seq, command, answer);
        true
    }

    fn capabilities(&self) -> Value {
        json!({
            "supportsConfigurationDoneRequest": true,
            "supportsConditionalBreakpoints": true,
            "supportsHitConditionalBreakpoints": true,
            "supportsLogPoints": true,
            "supportsEvaluateForHovers": true,
            "supportsRestartRequest": self.script.restart,
            "supportsTerminateRequest": self.script.terminate,
            "supportTerminateDebuggee": true,
            "exceptionBreakpointFilters": [
                {"filter": "panic", "label": "Rust Panics", "default": true},
                {"filter": "all", "label": "All Exceptions", "default": false,
                 "description": "Stops at every exception, caught or not."},
            ],
        })
    }

    fn launch(&mut self, seq: i64, arguments: &Value) -> bool {
        if self.script.run_in_terminal {
            let program = self.script.path.display().to_string();
            let cwd = self.script.path.parent().map(|p| p.display().to_string());
            self.waiting_launch = Some((seq, "launch".into()));
            let request_seq = self.next_seq();
            self.send(Message::Request {
                seq: request_seq,
                command: "runInTerminal".into(),
                arguments: json!({"kind": "integrated", "title": "fake", "cwd": cwd, "args": [program],
                    "env": {"FAKE": "1", "UNSET": null}}),
            });
            return true;
        }
        let spawned = match spawn_args(arguments).filter(|_| self.may_spawn) {
            Some(args) => match Command::new(&args[0]).args(&args[1..]).spawn() {
                Ok(child) => Some(child),
                Err(error) => {
                    self.respond(seq, "launch", Err(format!("could not start {}: {error}", args[0])));
                    return true;
                }
            },
            None => None,
        };
        let pid = spawned.as_ref().map(Child::id);
        self.child = spawned;
        self.respond(seq, "launch", Ok(Value::Null));
        let name = self.script.path.display().to_string();
        self.event(
            "process",
            json!({"name": name, "systemProcessId": pid, "startMethod": "launch"}),
        );
        true
    }

    fn terminal_answered(&mut self, success: bool, body: &Value) {
        let Some((seq, command)) = self.waiting_launch.take() else {
            return;
        };
        if !success {
            self.respond(seq, &command, Err("the terminal did not start the program".into()));
            return;
        }
        self.respond(seq, &command, Ok(Value::Null));
        let pid = body.get("processId").cloned().unwrap_or(Value::Null);
        self.event(
            "process",
            json!({"name": "in terminal", "systemProcessId": pid, "startMethod": "attachForSuspendedLaunch"}),
        );
        if std::mem::take(&mut self.begin_after_launch) {
            self.begin();
        }
    }

    fn set_breakpoints(&mut self, arguments: &Value) -> Value {
        let path = PathBuf::from(
            arguments
                .get("source")
                .and_then(|s| s.get("path"))
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        let asked: Vec<SourceBreakpoint> = arguments
            .get("breakpoints")
            .and_then(|b| serde_json::from_value(b.clone()).ok())
            .unwrap_or_default();
        let ours = path == self.script.path;
        let mut set = Vec::new();
        let mut answer = Vec::new();
        for breakpoint in asked {
            let id = self.next_breakpoint;
            self.next_breakpoint += 1;
            let verified = ours && self.script.lines.contains(&breakpoint.line);
            let mut reply = json!({"id": id, "verified": verified, "line": breakpoint.line,
                "source": {"path": path, "name": path.file_name().map(|n| n.to_string_lossy().into_owned())}});
            if !verified {
                reply["message"] = json!("no code on this line");
            }
            answer.push(reply);
            if verified {
                set.push(Set {
                    id,
                    asked: breakpoint,
                    hits: 0,
                });
            }
        }
        self.breakpoints.insert(path, set);
        json!({"breakpoints": answer})
    }

    fn begin(&mut self) {
        for thread in [MAIN, WORKER] {
            self.event("thread", json!({"reason": "started", "threadId": thread}));
        }
        for line in self.script.output.clone() {
            self.event("output", json!({"category": "stdout", "output": line}));
        }
        self.place = Place::At(0);
        self.run(0);
    }

    /// Runs from the line at `from` until a breakpoint holds, the panic, or the end.
    fn run(&mut self, from: usize) {
        for index in from..self.script.lines.len() {
            let line = self.script.lines[index];
            if let Some(hit) = self.hit(line) {
                self.place = Place::At(index);
                self.stopped("breakpoint", vec![hit]);
                return;
            }
        }
        self.after_last_line();
    }

    fn after_last_line(&mut self) {
        let catches = self.filters.iter().any(|f| f == "panic" || f == "all");
        if self.script.panics && catches && self.place != Place::Panicked {
            self.place = Place::Panicked;
            let seq = self.next_seq();
            self.send(Message::Event {
                seq,
                event: "stopped".into(),
                body: json!({"reason": "exception", "description": "panic", "text": "fake panic",
                    "threadId": MAIN, "allThreadsStopped": true}),
            });
            return;
        }
        self.finish();
    }

    fn finish(&mut self) {
        if self.place == Place::Ended {
            return;
        }
        self.place = Place::Ended;
        let code = self.script.exit_code;
        self.event("exited", json!({"exitCode": code}));
        self.event("terminated", Value::Null);
    }

    /// The breakpoint on `line` that stops the program now, counting the hit; log points print instead.
    fn hit(&mut self, line: u32) -> Option<i64> {
        let path = self.script.path.clone();
        let set = self
            .breakpoints
            .get_mut(&path)?
            .iter_mut()
            .find(|s| s.asked.line == line)?;
        set.hits += 1;
        if set.asked.condition.as_deref().is_some_and(|c| c.trim() == "false") {
            return None;
        }
        if let Some(wanted) = set.asked.hit_condition.as_deref() {
            let (at_least, number) = match wanted.trim().strip_prefix(">=") {
                Some(rest) => (true, rest.trim()),
                None => (false, wanted.trim()),
            };
            let number: u32 = number.parse().unwrap_or(0);
            if (at_least && set.hits < number) || (!at_least && set.hits != number) {
                return None;
            }
        }
        if let Some(message) = set.asked.log_message.clone() {
            let text = format!("{}\n", message.replace("{hits}", &set.hits.to_string()));
            self.event(
                "output",
                json!({"category": "console", "output": text, "line": line,
                "source": {"path": path}}),
            );
            return None;
        }
        Some(set.id)
    }

    fn stopped(&mut self, reason: &str, hits: Vec<i64>) {
        self.event(
            "stopped",
            json!({"reason": reason, "threadId": MAIN, "allThreadsStopped": true, "hitBreakpointIds": hits}),
        );
    }

    fn line_now(&self) -> u32 {
        match self.place {
            Place::At(index) => self.script.lines.get(index).copied().unwrap_or(0),
            _ => self.script.lines.last().copied().unwrap_or(0),
        }
    }

    fn hits(&self) -> u32 {
        self.breakpoints.values().flatten().map(|s| s.hits).sum()
    }

    fn stack_trace(&self, arguments: &Value) -> Value {
        let source = json!({"path": self.script.path, "name": self.script.path.file_name().map(|n| n.to_string_lossy().into_owned())});
        let first = self.script.lines.first().copied().unwrap_or(1);
        let frames = if arguments.get("threadId").and_then(Value::as_i64) == Some(WORKER) {
            json!([{"id": 3, "name": "worker::wait", "line": first, "column": 1, "source": source}])
        } else {
            json!([
                {"id": 1, "name": "fake::step", "line": self.line_now(), "column": 5, "source": source},
                {"id": 2, "name": "fake::main", "line": first, "column": 1, "source": source},
            ])
        };
        let total = frames.as_array().map_or(0, Vec::len);
        json!({"stackFrames": frames, "totalFrames": total})
    }

    fn variables(&self, arguments: &Value) -> Value {
        let reference = arguments.get("variablesReference").and_then(Value::as_i64).unwrap_or(0);
        let variables = match reference {
            POINT => json!([
                {"name": "x", "value": "1", "type": "i32", "variablesReference": 0, "evaluateName": "point.x"},
                {"name": "y", "value": "2", "type": "i32", "variablesReference": 0, "evaluateName": "point.y"},
                {"name": "inner", "value": "Inner { z: 3 }", "type": "Inner", "variablesReference": INNER,
                 "evaluateName": "point.inner"},
            ]),
            INNER => json!([{"name": "z", "value": "3", "type": "i32", "variablesReference": 0}]),
            r if (200..300).contains(&r) => {
                json!([{"name": "pc", "value": "0x0000000100003f40", "type": "u64", "variablesReference": 0}])
            }
            r if (100..200).contains(&r) => json!([
                {"name": "hits", "value": self.hits().to_string(), "type": "u32", "variablesReference": 0,
                 "evaluateName": "hits"},
                {"name": "line", "value": self.line_now().to_string(), "type": "u32", "variablesReference": 0,
                 "evaluateName": "line"},
                {"name": "point", "value": "Point { x: 1, y: 2, inner: Inner { z: 3 } }", "type": "Point",
                 "variablesReference": POINT, "evaluateName": "point"},
            ]),
            _ => json!([]),
        };
        json!({"variables": variables})
    }

    fn evaluate(&self, arguments: &Value) -> Result<Value, String> {
        let expression = arguments
            .get("expression")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        match expression {
            "hits" | "total" => Ok(json!({"result": self.hits().to_string(), "type": "u32", "variablesReference": 0})),
            "line" => Ok(json!({"result": self.line_now().to_string(), "type": "u32", "variablesReference": 0})),
            "point" => Ok(
                json!({"result": "Point { x: 1, y: 2, inner: Inner { z: 3 } }", "type": "Point",
                "variablesReference": POINT}),
            ),
            "point.x" => Ok(json!({"result": "1", "type": "i32", "variablesReference": 0})),
            other => Err(format!("use of undeclared identifier '{other}'")),
        }
    }

    fn execute(&mut self, seq: i64, command: &str) -> bool {
        if command == "restart" && !self.script.restart {
            self.respond(seq, command, Err("restart is not supported".into()));
            return true;
        }
        let place = self.place;
        let body = if command == "continue" {
            json!({"allThreadsContinued": true})
        } else {
            Value::Null
        };
        self.respond(seq, command, Ok(body));
        match (command, place) {
            ("pause", _) | (_, Place::Ended | Place::NotStarted) => {}
            ("restart", _) => {
                for set in self.breakpoints.values_mut().flatten() {
                    set.hits = 0;
                }
                self.place = Place::At(0);
                self.run(0);
            }
            (_, Place::Panicked) => self.finish(),
            ("continue", Place::At(index)) => self.run(index + 1),
            (step, Place::At(index)) => {
                let next = if step == "stepOut" {
                    self.script.lines.len()
                } else {
                    index + 1
                };
                if next < self.script.lines.len() {
                    self.place = Place::At(next);
                    self.stopped("step", Vec::new());
                } else {
                    self.after_last_line();
                }
            }
        }
        true
    }

    fn end_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn scopes(arguments: &Value) -> Value {
    let frame = arguments.get("frameId").and_then(Value::as_i64).unwrap_or(0);
    json!({"scopes": [
        {"name": "Locals", "presentationHint": "locals", "variablesReference": 100 + frame, "expensive": false},
        {"name": "Registers", "presentationHint": "registers", "variablesReference": 200 + frame, "expensive": true},
    ]})
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}
