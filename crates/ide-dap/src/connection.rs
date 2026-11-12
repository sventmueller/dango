//! One connection to an adapter: requests with deadlines, events, and the adapter's own requests to the client.
//!
//! Three threads per connection, as for language servers: one reads and dispatches, one writes from a bounded queue
//! so a caller never waits on a full pipe, one fails requests whose deadline passed.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::error::Error;
use crate::protocol::{self, Message};

/// Messages that may wait for the writer; more means the adapter stopped reading.
const QUEUE: usize = 4096;

/// Called once with a response's body.
pub(crate) type Answer = Box<dyn FnOnce(Result<Value, Error>) + Send>;

/// What the adapter sends on its own.
pub(crate) trait Handler: Send + Sync {
    fn event(&self, event: &str, body: Value);
    fn request(&self, command: &str, arguments: Value, responder: Responder);
    /// The stream ended or broke; every waiting request has been failed.
    fn closed(&self);
}

struct Call {
    command: String,
    answer: Answer,
    deadline: Instant,
}

#[derive(Default)]
struct Pending {
    calls: HashMap<i64, Call>,
    closed: bool,
}

enum Out {
    Message(Vec<u8>),
    End,
}

/// Writes both directions of a session as lines of JSON, for fixtures replayed in tests.
struct Recorder(Mutex<File>);

impl Recorder {
    fn write(&self, from: &str, message: &Value) {
        let line = json!({"from": from, "message": message});
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(file, "{line}");
    }
}

/// One connection.
pub(crate) struct Connection {
    out: SyncSender<Out>,
    pending: Mutex<Pending>,
    wake: Condvar,
    seq: AtomicI64,
    recorder: Option<Recorder>,
}

impl Connection {
    /// Starts reading `input` and writing `output`; `handler` hears what the adapter sends.
    pub(crate) fn start(
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
        handler: Arc<dyn Handler>,
        name: &str,
        record: Option<File>,
    ) -> io::Result<Arc<Connection>> {
        let (out, queue) = sync_channel(QUEUE);
        let connection = Arc::new(Connection {
            out,
            pending: Mutex::default(),
            wake: Condvar::new(),
            seq: AtomicI64::new(1),
            recorder: record.map(|file| Recorder(Mutex::new(file))),
        });
        std::thread::Builder::new()
            .name(format!("dap-write-{name}"))
            .spawn(move || write_all(output, &queue))?;
        let timer = connection.clone();
        std::thread::Builder::new()
            .name(format!("dap-timer-{name}"))
            .spawn(move || timer.expire())?;
        let reader = Arc::downgrade(&connection);
        if let Err(error) = std::thread::Builder::new()
            .name(format!("dap-read-{name}"))
            .spawn(move || read_all(input, &reader, &*handler))
        {
            connection.close();
            return Err(error);
        }
        Ok(connection)
    }

    /// Sends a request; `answer` hears the body, an error or the timeout exactly once. When this returns an error,
    /// `answer` is dropped without a call.
    pub(crate) fn request(
        &self,
        command: &str,
        arguments: Value,
        timeout: Duration,
        answer: Answer,
    ) -> Result<i64, Error> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        {
            let mut pending = self.lock();
            if pending.closed {
                return Err(Error::Ended);
            }
            let deadline = Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(365 * 24 * 3600));
            let command = command.to_string();
            pending.calls.insert(
                seq,
                Call {
                    command,
                    answer,
                    deadline,
                },
            );
        }
        self.wake.notify_all();
        let message = Message::Request {
            seq,
            command: command.to_string(),
            arguments,
        };
        if let Err(error) = self.send(&message)
            && self.lock().calls.remove(&seq).is_some()
        {
            return Err(error);
        }
        Ok(seq)
    }

    /// Whether the connection still carries messages.
    pub(crate) fn is_open(&self) -> bool {
        !self.lock().closed
    }

    fn send(&self, message: &Message) -> Result<(), Error> {
        if let Some(recorder) = &self.recorder {
            recorder.write("client", &message.to_json());
        }
        match self.out.try_send(Out::Message(message.to_frame())) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(Error::Congested),
            Err(TrySendError::Disconnected(_)) => Err(Error::Ended),
        }
    }

    fn respond(&self, request_seq: i64, command: String, answer: Result<Value, String>) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let (success, message, body) = match answer {
            Ok(body) => (true, None, body),
            Err(message) => (false, Some(message), Value::Null),
        };
        let _ = self.send(&Message::Response {
            seq,
            request_seq,
            success,
            command,
            message,
            body,
        });
    }

    fn dispatch(self: &Arc<Self>, body: &[u8], handler: &dyn Handler) {
        let Ok(message) = protocol::parse(body) else {
            return;
        };
        if let Some(recorder) = &self.recorder {
            recorder.write("adapter", &message.to_json());
        }
        match message {
            Message::Event { event, body, .. } => contained(|| handler.event(&event, body)),
            Message::Request {
                seq,
                command,
                arguments,
            } => {
                let responder = Responder {
                    seq,
                    command: Some(command.clone()),
                    connection: Arc::downgrade(self),
                };
                contained(|| handler.request(&command, arguments, responder));
            }
            Message::Response {
                request_seq,
                success,
                message,
                body,
                ..
            } => {
                let Some(call) = self.lock().calls.remove(&request_seq) else {
                    return;
                };
                let answer = if success {
                    Ok(body)
                } else {
                    Err(Error::Adapter {
                        command: call.command,
                        message: message.unwrap_or_default(),
                    })
                };
                contained(|| (call.answer)(answer));
            }
        }
    }

    /// Fails every waiting request and ends the adapter's input; nothing more can be sent.
    pub(crate) fn close(&self) {
        let calls: Vec<Call> = {
            let mut pending = self.lock();
            pending.closed = true;
            pending.calls.drain().map(|(_, call)| call).collect()
        };
        self.wake.notify_all();
        let _ = self.out.try_send(Out::End);
        for call in calls {
            contained(|| (call.answer)(Err(Error::Ended)));
        }
    }

    fn expire(&self) {
        let mut pending = self.lock();
        loop {
            if pending.closed {
                return;
            }
            let now = Instant::now();
            let due: Vec<i64> = pending
                .calls
                .iter()
                .filter(|(_, call)| call.deadline <= now)
                .map(|(seq, _)| *seq)
                .collect();
            if !due.is_empty() {
                let calls: Vec<Call> = due.iter().filter_map(|seq| pending.calls.remove(seq)).collect();
                drop(pending);
                for call in calls {
                    contained(|| (call.answer)(Err(Error::Timeout)));
                }
                pending = self.lock();
                continue;
            }
            pending = match pending.calls.values().map(|call| call.deadline).min() {
                Some(deadline) => {
                    self.wake
                        .wait_timeout(pending, deadline.saturating_duration_since(now))
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                None => self.wake.wait(pending).unwrap_or_else(|e| e.into_inner()),
            };
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Answers one request from the adapter; dropped unanswered, it answers with a failure.
pub(crate) struct Responder {
    seq: i64,
    command: Option<String>,
    connection: Weak<Connection>,
}

impl Responder {
    pub(crate) fn ok(mut self, body: Value) {
        self.answer(Ok(body));
    }

    pub(crate) fn fail(mut self, message: &str) {
        self.answer(Err(message.to_string()));
    }

    fn answer(&mut self, answer: Result<Value, String>) {
        if let (Some(command), Some(connection)) = (self.command.take(), self.connection.upgrade()) {
            connection.respond(self.seq, command, answer);
        }
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.answer(Err("the request was not answered".to_string()));
    }
}

fn read_all(input: impl Read, connection: &Weak<Connection>, handler: &dyn Handler) {
    let mut input = BufReader::with_capacity(64 * 1024, input);
    while let Ok(Some(body)) = ide_process::framing::read(&mut input) {
        let Some(connection) = connection.upgrade() else {
            return;
        };
        connection.dispatch(&body, handler);
    }
    if let Some(connection) = connection.upgrade() {
        connection.close();
    }
    contained(|| handler.closed());
}

fn write_all(mut output: impl Write, queue: &Receiver<Out>) {
    while let Ok(Out::Message(bytes)) = queue.recv() {
        if output.write_all(&bytes).and_then(|()| output.flush()).is_err() {
            return;
        }
    }
}

/// Runs `f`; a panic in a caller's callback costs that callback, not the connection.
pub(crate) fn contained(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

#[cfg(test)]
mod tests {
    use std::io::{PipeReader, PipeWriter};
    use std::sync::mpsc::channel;

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);

    #[derive(Default)]
    struct Recorded {
        events: Mutex<Vec<String>>,
    }

    impl Handler for Recorded {
        fn event(&self, event: &str, _: Value) {
            self.events.lock().unwrap().push(event.to_string());
        }

        fn request(&self, command: &str, arguments: Value, responder: Responder) {
            if command == "runInTerminal" {
                responder.ok(json!({"processId": arguments["n"]}));
            }
        }

        fn closed(&self) {}
    }

    struct Peer {
        from_client: BufReader<PipeReader>,
        to_client: Option<PipeWriter>,
    }

    impl Peer {
        fn next(&mut self) -> Message {
            protocol::parse(&ide_process::framing::read(&mut self.from_client).unwrap().unwrap()).unwrap()
        }

        fn send(&mut self, message: &Message) {
            self.to_client.as_mut().unwrap().write_all(&message.to_frame()).unwrap();
        }
    }

    fn connect() -> (Arc<Connection>, Peer, Arc<Recorded>) {
        let (client_in, to_client) = std::io::pipe().unwrap();
        let (from_client, client_out) = std::io::pipe().unwrap();
        let handler = Arc::new(Recorded::default());
        let connection = Connection::start(client_in, client_out, handler.clone(), "test", None).unwrap();
        let peer = Peer {
            from_client: BufReader::new(from_client),
            to_client: Some(to_client),
        };
        (connection, peer, handler)
    }

    fn response(request_seq: i64, success: bool, body: Value) -> Message {
        Message::Response {
            seq: 100 + request_seq,
            request_seq,
            success,
            command: "x".into(),
            message: (!success).then(|| "it failed".to_string()),
            body,
        }
    }

    #[test]
    fn responses_reach_their_requests_and_failures_carry_the_adapters_words() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        let first = connection
            .request("threads", Value::Null, WAIT, Box::new(move |a| tx.send(a).unwrap()))
            .unwrap();
        let second = connection
            .request("launch", json!({}), WAIT, Box::new(move |a| tx2.send(a).unwrap()))
            .unwrap();
        assert!(matches!(peer.next(), Message::Request { ref command, .. } if command == "threads"));
        assert!(matches!(peer.next(), Message::Request { ref command, .. } if command == "launch"));
        peer.send(&response(second, false, Value::Null));
        peer.send(&response(first, true, json!({"threads": []})));
        assert_eq!(
            rx.recv_timeout(WAIT).unwrap(),
            Err(Error::Adapter {
                command: "launch".into(),
                message: "it failed".into()
            })
        );
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Ok(json!({"threads": []})));
    }

    #[test]
    fn a_late_answer_times_out_once() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        let seq = connection
            .request(
                "evaluate",
                Value::Null,
                Duration::from_millis(50),
                Box::new(move |a| tx.send(a).unwrap()),
            )
            .unwrap();
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::Timeout));
        peer.next();
        peer.send(&response(seq, true, Value::Null));
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn the_adapters_requests_are_answered_and_events_heard() {
        let (_connection, mut peer, handler) = connect();
        peer.send(&Message::Event {
            seq: 1,
            event: "initialized".into(),
            body: Value::Null,
        });
        peer.send(&Message::Request {
            seq: 2,
            command: "runInTerminal".into(),
            arguments: json!({"n": 7}),
        });
        let Message::Response {
            request_seq,
            success,
            body,
            ..
        } = peer.next()
        else {
            panic!("no response");
        };
        assert_eq!((request_seq, success, body), (2, true, json!({"processId": 7})));
        peer.send(&Message::Request {
            seq: 3,
            command: "unknown".into(),
            arguments: Value::Null,
        });
        assert!(matches!(peer.next(), Message::Response { success: false, .. }));
        assert_eq!(handler.events.lock().unwrap().as_slice(), ["initialized"]);
    }

    #[test]
    fn the_end_of_the_stream_fails_what_waits_and_refuses_more() {
        let (connection, mut peer, _) = connect();
        let (tx, rx) = channel();
        connection
            .request("next", Value::Null, WAIT, Box::new(move |a| tx.send(a).unwrap()))
            .unwrap();
        peer.to_client = None;
        assert_eq!(rx.recv_timeout(WAIT).unwrap(), Err(Error::Ended));
        let deadline = Instant::now() + WAIT;
        while connection.is_open() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            connection.request("next", Value::Null, WAIT, Box::new(|_| panic!("not called"))),
            Err(Error::Ended)
        );
    }
}
