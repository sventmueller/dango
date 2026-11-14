//! One connection to an adapter on the shared transport (`ide-rpc`): requests with deadlines, events, and the
//! adapter's own requests to the client.

use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use ide_core::LockIgnoringPoison;
use ide_rpc::{RequestId, SendError, Transport};
use serde_json::{Value, json};

use crate::error::Error;
use crate::protocol::{self, Message};

pub(crate) use ide_rpc::contained;

/// Called once with a response's body.
pub(crate) type Answer = Box<dyn FnOnce(Result<Value, Error>) + Send>;

/// What the adapter sends on its own.
pub(crate) trait Handler: Send + Sync {
    fn event(&self, event: &str, body: Value);
    fn request(&self, command: &str, arguments: Value, responder: Responder);
    /// The stream ended or broke; every waiting request has been failed.
    fn closed(&self);
}

/// A request waiting for its response.
struct Waiting {
    command: String,
    answer: Answer,
}

impl ide_rpc::Call for Waiting {
    fn closed(self) {
        (self.answer)(Err(Error::Ended));
    }

    fn expired(self, _: RequestId, _: &Transport<Self>) {
        (self.answer)(Err(Error::Timeout));
    }
}

/// Writes both directions of a session as lines of JSON, for fixtures replayed in tests.
struct Recorder(Mutex<File>);

impl Recorder {
    fn write(&self, from: &str, message: &Value) {
        let line = json!({"from": from, "message": message});
        let mut file = self.0.lock_ignoring_poison();
        if let Err(error) = writeln!(file, "{line}") {
            ide_core::diag::report("dap: recording", error);
        }
    }
}

/// Records `message` when recording, and queues it.
fn send(transport: &Transport<Waiting>, recorder: Option<&Recorder>, message: &Message) -> Result<(), Error> {
    if let Some(recorder) = recorder {
        recorder.write("client", &message.to_json());
    }
    transport.send(message.to_frame()).map_err(error_of)
}

fn error_of(error: SendError) -> Error {
    match error {
        SendError::Congested => Error::Congested,
        SendError::Closed => Error::Ended,
    }
}

/// One connection.
pub(crate) struct Connection {
    transport: Arc<Transport<Waiting>>,
    recorder: Option<Arc<Recorder>>,
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
        let recorder = record.map(|file| Arc::new(Recorder(Mutex::new(file))));
        let dispatch = Dispatch {
            handler,
            recorder: recorder.clone(),
        };
        let transport = Transport::start(input, output, Arc::new(dispatch), "dap", name)?;
        Ok(Arc::new(Connection { transport, recorder }))
    }

    /// Sends a request; `answer` hears the body, an error or the timeout exactly once. When this returns an error,
    /// `answer` is dropped without a call.
    pub(crate) fn request(
        &self,
        command: &str,
        arguments: Value,
        timeout: Duration,
        answer: Answer,
    ) -> Result<RequestId, Error> {
        let transport = &self.transport;
        let seq = transport.next_id();
        let message = Message::Request {
            seq: seq.get(),
            command: command.to_string(),
            arguments,
        };
        if let Some(recorder) = &self.recorder {
            recorder.write("client", &message.to_json());
        }
        let waiting = Waiting {
            command: command.to_string(),
            answer,
        };
        transport
            .call(seq, message.to_frame(), timeout, waiting)
            .map_err(error_of)?;
        Ok(seq)
    }

    /// Whether the connection still carries messages.
    pub(crate) fn is_open(&self) -> bool {
        self.transport.is_open()
    }

    /// Fails every waiting request and ends the adapter's input; nothing more can be sent.
    pub(crate) fn close(&self) {
        self.transport.close();
    }
}

/// Hands what the adapter sends to the handler or to the request it answers.
struct Dispatch {
    handler: Arc<dyn Handler>,
    recorder: Option<Arc<Recorder>>,
}

impl ide_rpc::Receiver<Waiting> for Dispatch {
    fn receive(&self, transport: &Arc<Transport<Waiting>>, body: &[u8]) {
        let message = match protocol::parse(body) {
            Ok(message) => message,
            Err(error) => return ide_core::diag::report("dap: undecodable message", error),
        };
        if let Some(recorder) = &self.recorder {
            recorder.write("adapter", &message.to_json());
        }
        match message {
            Message::Event { event, body, .. } => contained(|| self.handler.event(&event, body)),
            Message::Request {
                seq,
                command,
                arguments,
            } => {
                let responder = Responder::new(Arc::downgrade(transport), self.recorder.clone(), seq, command.clone());
                contained(|| self.handler.request(&command, arguments, responder));
            }
            Message::Response {
                request_seq,
                success,
                message,
                body,
                ..
            } => {
                let Some(waiting) = transport.take(RequestId::new(request_seq)) else {
                    return;
                };
                let answer = if success {
                    Ok(body)
                } else {
                    Err(Error::Adapter {
                        command: waiting.command,
                        message: message.unwrap_or_default(),
                    })
                };
                contained(|| (waiting.answer)(answer));
            }
        }
    }

    fn ended(&self) {
        self.handler.closed();
    }
}

/// Answers one request from the adapter; dropped unanswered, it answers with a failure.
pub(crate) struct Responder(ide_rpc::Responder<String>);

impl Responder {
    fn new(
        transport: Weak<Transport<Waiting>>,
        recorder: Option<Arc<Recorder>>,
        seq: i64,
        command: String,
    ) -> Responder {
        let answer = move |answer: Result<Value, String>| {
            let Some(transport) = transport.upgrade() else {
                return;
            };
            let (success, message, body) = match answer {
                Ok(body) => (true, None, body),
                Err(message) => (false, Some(message), Value::Null),
            };
            let response = Message::Response {
                seq: transport.next_id().get(),
                request_seq: seq,
                success,
                command,
                message,
                body,
            };
            let _ = send(&transport, recorder.as_deref(), &response);
        };
        Responder(ide_rpc::Responder::new(
            answer,
            "the request was not answered".to_string(),
        ))
    }

    pub(crate) fn ok(self, body: Value) {
        self.0.ok(body);
    }

    pub(crate) fn fail(self, message: &str) {
        self.0.fail(message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, PipeReader, PipeWriter};
    use std::sync::mpsc::channel;
    use std::time::Instant;

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
        peer.send(&response(second.get(), false, Value::Null));
        peer.send(&response(first.get(), true, json!({"threads": []})));
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
        peer.send(&response(seq.get(), true, Value::Null));
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
